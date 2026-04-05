//! `#[task(...)]` attribute proc macro.
//!
//! Parses a task annotation and auto-registers it via `inventory::submit!`.
//! Replaces the function with a constant of a generated struct to allow
//! `.dispatch()` and `.delay()` syntax.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    parse::Parse, parse::ParseStream, Ident, LitStr, LitInt, Token, ItemFn, Result,
};

/// Parsed contents of `#[task(...)]`
#[derive(Debug)]
pub struct TaskAttr {
    pub name: Option<String>,
    pub queue: Option<String>,
    pub retries: Option<u32>,
    pub timeout: Option<u32>,
}

impl Parse for TaskAttr {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut name: Option<String> = None;
        let mut queue: Option<String> = None;
        let mut retries: Option<u32> = None;
        let mut timeout: Option<u32> = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;

            match key.to_string().as_str() {
                "name" => {
                    let lit: LitStr = input.parse()?;
                    name = Some(lit.value());
                }
                "queue" => {
                    let lit: LitStr = input.parse()?;
                    queue = Some(lit.value());
                }
                "retries" => {
                    let lit: LitInt = input.parse()?;
                    retries = Some(lit.base10_parse()?);
                }
                "timeout" => {
                    let lit: LitInt = input.parse()?;
                    timeout = Some(lit.base10_parse()?);
                }
                other => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!(
                            "unknown task attribute `{}`. Expected: name, queue, retries, timeout",
                            other
                        ),
                    ));
                }
            }

            // consume optional trailing comma
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(TaskAttr { name, queue, retries, timeout })
    }
}

pub fn expand_task(attr: TokenStream, item: TokenStream) -> TokenStream {
    let task_attr = match syn::parse2::<TaskAttr>(attr) {
        Ok(a) => a,
        Err(e) => return e.to_compile_error(),
    };
    let handler_fn = match syn::parse2::<ItemFn>(item) {
        Ok(f) => f,
        Err(e) => return e.to_compile_error(),
    };

    let fn_name = &handler_fn.sig.ident;
    let fn_vis = &handler_fn.vis;
    let fn_sig = &handler_fn.sig;
    let fn_block = &handler_fn.block;
    
    // Extract arg names and types for the dispatch method
    let mut arg_names = Vec::new();
    let mut arg_types = Vec::new();
    for arg in &handler_fn.sig.inputs {
        if let syn::FnArg::Typed(pat_type) = arg {
            if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                arg_names.push(&pat_ident.ident);
                arg_types.push(&pat_type.ty);
            }
        }
    }

    let inner_fn_name = syn::Ident::new(&format!("__floz_task_inner_{}", fn_name), fn_name.span());
    let struct_name = syn::Ident::new(&format!("__FlozTask_{}", fn_name), fn_name.span());
    let builder_name = syn::Ident::new(&format!("__FlozTaskBuilder_{}", fn_name), fn_name.span());

    let task_name = task_attr.name.unwrap_or_else(|| fn_name.to_string());
    let queue_name = task_attr.queue.unwrap_or_else(|| "default".to_string());
    let max_retries = task_attr.retries.unwrap_or(0);

    let expanded = quote! {
        // Inner function containing the original logic
        #fn_vis async fn #inner_fn_name(#(#arg_names: #arg_types),*) -> ::core::result::Result<(), ::floz::worker::TaskError> {
            #fn_block
        }

        // --- The typed builder for dispatching with delay/schedule ---
        #[allow(non_camel_case_types)]
        #fn_vis struct #builder_name {
            eta: ::core::option::Option<::floz::chrono::DateTime<::floz::chrono::Utc>>,
        }

        impl #builder_name {
            pub fn delay(mut self, delay: ::std::time::Duration) -> Self {
                self.eta = ::core::option::Option::Some(::floz::chrono::Utc::now() + ::floz::chrono::Duration::from_std(delay).unwrap_or(::floz::chrono::Duration::zero()));
                self
            }

            pub fn schedule(mut self, time: ::floz::chrono::DateTime<::floz::chrono::Utc>) -> Self {
                self.eta = ::core::option::Option::Some(time);
                self
            }

            pub async fn dispatch(self, ctx: &::floz::app::AppContext, #(#arg_names: #arg_types),*) -> ::core::result::Result<(), ::floz::worker::TaskError> {
                let args_json = ::floz::serde_json::to_value((#(#arg_names,)*))?;
                let mut msg = ::floz::worker::TaskMessage::new(#task_name, #queue_name, args_json, #max_retries);
                msg.eta = self.eta;
                
                // Get the redis broker from context cache
                // Assume the cache wrapped connection can be used as a broker or we create one.
                // Wait, AppContext doesn't have `broker`. Hmm. Worker needs to enqueue. 
                // Let's assume `ctx.enqueue(msg).await` exists? 
                // Let's add `enqueue` to AppContext!
                
                ctx.enqueue(msg).await
            }
        }

        // --- The constant struct replacement ---
        #[allow(non_camel_case_types)]
        #fn_vis struct #struct_name;

        impl #struct_name {
            pub async fn dispatch(&self, ctx: &::floz::app::AppContext, #(#arg_names: #arg_types),*) -> ::core::result::Result<(), ::floz::worker::TaskError> {
                #builder_name { eta: ::core::option::Option::None }.dispatch(ctx, #(#arg_names),*).await
            }

            pub fn delay(&self, delay: ::std::time::Duration) -> #builder_name {
                #builder_name { eta: ::core::option::Option::None }.delay(delay)
            }
            
            pub fn schedule(&self, time: ::floz::chrono::DateTime<::floz::chrono::Utc>) -> #builder_name {
                #builder_name { eta: ::core::option::Option::None }.schedule(time)
            }
        }

        // The exposed constant
        #[allow(non_upper_case_globals)]
        #fn_vis const #fn_name: #struct_name = #struct_name;

        // Auto-register this task via inventory
        ::floz::inventory::submit! {
            ::floz::worker::TaskEntry::new(&#fn_name)
        }

        // Implement TaskDef for the struct
        impl ::floz::worker::TaskDef for #struct_name {
            fn name(&self) -> &'static str {
                #task_name
            }

            fn default_queue(&self) -> &'static str {
                #queue_name
            }

            fn max_retries(&self) -> u32 {
                #max_retries
            }

            fn call<'a>(
                &'a self,
                ctx: ::floz::app::AppContext,
                args: ::floz::serde_json::Value,
            ) -> ::std::pin::Pin<::std::boxed::Box<dyn ::std::future::Future<Output = ::core::result::Result<(), ::floz::worker::TaskError>> + Send + 'a>> {
                // We assume args deserialize to a tuple matching the fn_sig inputs
                // A helper trait or direct parsing is needed.
                // For MVP, if it takes (T1, T2) we deserialize that tuple.
                ::std::boxed::Box::pin(async move {
                    // Extract fields from json array
                    let parsed_args: (#(#arg_types,)*) = ::floz::serde_json::from_value(args)?;
                    let (#(#arg_names,)*) = parsed_args;
                    #inner_fn_name(#(#arg_names),*).await
                })
            }
        }
    };

    expanded
}
