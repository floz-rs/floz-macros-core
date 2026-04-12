//! `#[channel_gate(...)]` attribute proc macro.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse::Parse, parse::ParseStream, Ident, LitStr, Token, ItemFn, Result, FnArg, PatType, PatIdent};

pub struct ChannelAttr {
    pub pattern: String,
}

impl Parse for ChannelAttr {
    fn parse(input: ParseStream) -> Result<Self> {
        let lit: LitStr = input.parse()?;
        Ok(ChannelAttr { pattern: lit.value() })
    }
}

pub fn expand_channel_gate(attr: TokenStream, item: TokenStream) -> TokenStream {
    let channel_attr = match syn::parse2::<ChannelAttr>(attr) {
        Ok(a) => a,
        Err(e) => return e.to_compile_error(),
    };
    let handler_fn = match syn::parse2::<ItemFn>(item) {
        Ok(f) => f,
        Err(e) => return e.to_compile_error(),
    };

    let fn_name = &handler_fn.sig.ident;
    let fn_vis = &handler_fn.vis;
    let fn_attrs = &handler_fn.attrs;
    let fn_sig = &handler_fn.sig;
    let fn_block = &handler_fn.block;
    let original_pattern = &channel_attr.pattern;

    // Collect variables to inject
    let mut param_extractors = Vec::new();
    let mut call_args = Vec::new();

    for arg in &handler_fn.sig.inputs {
        if let FnArg::Typed(PatType { pat, ty, .. }) = arg {
            if let syn::Pat::Ident(PatIdent { ident, .. }) = &**pat {
                let id_str = ident.to_string();
                if id_str == "ctx" || id_str == "_ctx" {
                    call_args.push(quote! { __ctx });
                } else {
                    // Extract from HashMap
                    let ty_str = quote! { #ty }.to_string();
                    if ty_str.contains("String") {
                        param_extractors.push(quote! {
                            let #ident = __vars.get(#id_str).cloned().unwrap_or_default();
                        });
                        call_args.push(quote! { #ident });
                    } else {
                        // Attempt to parse non-string types
                        param_extractors.push(quote! {
                            let #ident = __vars.get(#id_str)
                                .and_then(|v| v.parse().ok())
                                .unwrap_or_default();
                        });
                        call_args.push(quote! { #ident });
                    }
                }
            }
        }
    }

    let wrapper_fn_name = syn::Ident::new(
        &format!("__floz_call_gate_{}", fn_name),
        fn_name.span(),
    );

    let expanded = quote! {
        #(#fn_attrs)*
        #fn_vis #fn_sig #fn_block

        #[allow(non_snake_case)]
        fn #wrapper_fn_name(
            __ctx: ::floz::prelude::Context, 
            __vars: ::std::collections::HashMap<String, String>
        ) -> ::std::pin::Pin<Box<dyn ::std::future::Future<Output = bool> + Send + 'static>> {
            #(#param_extractors)*
            
            ::std::boxed::Box::pin(async move {
                #fn_name(#(#call_args),*).await
            })
        }

        ::floz::inventory::submit! {
            ::floz::web::channels::ChannelGateEntry::new(
                #original_pattern,
                #wrapper_fn_name
            )
        }
    };

    expanded
}
