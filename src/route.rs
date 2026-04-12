//! `#[route(...)]` attribute proc macro.
//!
//! Parses a single annotation that defines everything about a handler:
//! HTTP method, URL path, tag, description, response specs — and auto-registers it
//! via `inventory::submit!` so no manual route wiring is needed.
//!
//! # Example
//!
//! ```ignore
//! #[route(
//!     get: "/users/:id",
//!     tag: "Users",
//!     desc: "Get a user by ID",
//!     resps: [
//!         (200, "User found"),
//!         (404, "User not found"),
//!     ],
//! )]
//! async fn get_user(ctx: Ctx, Path(id): Path<i32>) -> Result<Json<User>, ApiError> {
//!     // ...
//! }
//! ```

use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    parse::Parse, parse::ParseStream, Ident, LitStr, LitInt, Token, ItemFn, Result,
    bracketed, parenthesized,
};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Attribute parsing
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A single response specification: (status_code, description, optional content_type)
pub struct ResponseSpec {
    pub status: u16,
    pub description: String,
    pub content_type: Option<String>,
    pub schema_type: Option<syn::TypePath>,
}

/// Parsed contents of `#[route(...)]`
pub struct RouteAttr {
    pub method: HttpMethod,
    pub path: String,
    pub tag: Option<String>,
    pub desc: Option<String>,
    pub resps: Vec<ResponseSpec>,
    pub auth: Option<String>,
    pub rate: Option<String>,
    pub wrap: Vec<syn::Expr>,
    pub permissions: Vec<String>,
    pub cache_ttl: Option<u64>,
    pub cache_watch: Vec<String>,
}

#[derive(Clone, Copy)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

impl HttpMethod {
    fn as_str(&self) -> &'static str {
        match self {
            HttpMethod::Get => "get",
            HttpMethod::Post => "post",
            HttpMethod::Put => "put",
            HttpMethod::Patch => "patch",
            HttpMethod::Delete => "delete",
        }
    }

    fn as_ident(&self) -> proc_macro2::Ident {
        proc_macro2::Ident::new(self.as_str(), proc_macro2::Span::call_site())
    }
}

/// Parse a single response tuple: (200, "description") or (200, "description", "text/html")
impl Parse for ResponseSpec {
    fn parse(input: ParseStream) -> Result<Self> {
        let content;
        parenthesized!(content in input);

        let status_lit: LitInt = content.parse()?;
        let status: u16 = status_lit.base10_parse()?;

        content.parse::<Token![,]>()?;
        let desc_lit: LitStr = content.parse()?;
        let description = desc_lit.value();

        let mut content_type = None;
        let mut schema_type = None;

        if content.peek(Token![,]) {
            content.parse::<Token![,]>()?;
            if !content.is_empty() {
                if content.peek(LitStr) {
                    let ct_lit: LitStr = content.parse()?;
                    content_type = Some(ct_lit.value());
                } else {
                    // Try to parse as TypePath e.g., Json<User>
                    let path: syn::TypePath = content.parse()?;
                    schema_type = Some(path);
                }
            }
        }

        Ok(ResponseSpec { status, description, content_type, schema_type })
    }
}

impl Parse for RouteAttr {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut method: Option<HttpMethod> = None;
        let mut path: Option<String> = None;
        let mut tag: Option<String> = None;
        let mut desc: Option<String> = None;
        let mut resps: Vec<ResponseSpec> = Vec::new();
        let mut auth: Option<String> = None;
        let mut rate: Option<String> = None;
        let mut permissions: Vec<String> = Vec::new();
        let mut wrap: Vec<syn::Expr> = Vec::new();
        let mut cache_ttl: Option<u64> = None;
        let mut cache_watch: Vec<String> = Vec::new();

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            let key_str = key.to_string();

            let is_bare_auth = key_str == "auth" && !input.peek(Token![:]);

            // `cache(...)` uses parenthesized syntax — no colon. `auth` can lack a colon if it means "required".
            if key_str != "cache" && !is_bare_auth {
                input.parse::<Token![:]>()?;
            }

            if is_bare_auth {
                auth = Some("required".to_string());
            } else {
                match key_str.as_str() {
                    "get" => {
                        method = Some(HttpMethod::Get);
                        let lit: LitStr = input.parse()?;
                        path = Some(lit.value());
                    }
                    "post" => {
                        method = Some(HttpMethod::Post);
                        let lit: LitStr = input.parse()?;
                        path = Some(lit.value());
                    }
                    "put" => {
                        method = Some(HttpMethod::Put);
                        let lit: LitStr = input.parse()?;
                        path = Some(lit.value());
                    }
                    "patch" => {
                        method = Some(HttpMethod::Patch);
                        let lit: LitStr = input.parse()?;
                        path = Some(lit.value());
                    }
                    "delete" => {
                        method = Some(HttpMethod::Delete);
                        let lit: LitStr = input.parse()?;
                        path = Some(lit.value());
                    }
                    "tag" => {
                        let lit: LitStr = input.parse()?;
                        tag = Some(lit.value());
                    }
                    "desc" => {
                        let lit: LitStr = input.parse()?;
                        desc = Some(lit.value());
                    }
                    "resps" => {
                        let content;
                        bracketed!(content in input);
                        while !content.is_empty() {
                            let resp: ResponseSpec = content.parse()?;
                            resps.push(resp);
                            if content.peek(Token![,]) {
                                content.parse::<Token![,]>()?;
                            }
                        }
                    }
                    "auth" => {
                        // auth: jwt | api_key | none (parsed as ident, not string)
                        let ident: Ident = input.parse()?;
                        auth = Some(ident.to_string());
                    }
                    "rate" => {
                        let lit: LitStr = input.parse()?;
                        rate = Some(lit.value());
                    }
                    "permissions" => {
                        let content;
                        bracketed!(content in input);
                        while !content.is_empty() {
                            let lit: LitStr = content.parse()?;
                            permissions.push(lit.value());
                            if content.peek(Token![,]) {
                                content.parse::<Token![,]>()?;
                            }
                        }
                    }
                    "middleware" | "wrap" => {
                        let content;
                        syn::bracketed!(content in input);
                        while !content.is_empty() {
                            let expr: syn::Expr = content.parse()?;
                            wrap.push(expr);
                            if content.peek(Token![,]) {
                                content.parse::<Token![,]>()?;
                            }
                        }
                    }
                    "cache" => {
                        let content;
                        parenthesized!(content in input);
                        while !content.is_empty() {
                            let config_key: Ident = content.parse()?;
                            content.parse::<Token![=]>()?;
                            if config_key == "ttl" {
                                let ttl_lit: LitInt = content.parse()?;
                                cache_ttl = Some(ttl_lit.base10_parse()?);
                            } else if config_key == "watch" {
                                let array_content;
                                syn::bracketed!(array_content in content);
                                while !array_content.is_empty() {
                                    let lit: LitStr = array_content.parse()?;
                                    cache_watch.push(lit.value());
                                    if array_content.peek(Token![,]) {
                                        array_content.parse::<Token![,]>()?;
                                    }
                                }
                            } else {
                                return Err(syn::Error::new(config_key.span(), "Unknown cache parameter (expected ttl or watch)"));
                            }
                            if content.peek(Token![,]) {
                                content.parse::<Token![,]>()?;
                            }
                        }
                    }
                    other => {
                        return Err(syn::Error::new(
                            key.span(),
                            format!(
                                "unknown route attribute `{}`. Expected: get/post/put/patch/delete, tag, desc, resps, auth, permissions, rate, middleware, wrap, cache",
                                other
                            ),
                        ));
                    }
                }
            }

            // consume optional trailing comma
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        let method = method.ok_or_else(|| {
            syn::Error::new(
                proc_macro2::Span::call_site(),
                "#[route] requires an HTTP method (get, post, put, patch, delete)",
            )
        })?;

        let path = path.ok_or_else(|| {
            syn::Error::new(
                proc_macro2::Span::call_site(),
                "#[route] requires a path string",
            )
        })?;

        Ok(RouteAttr { method, path, tag, desc, resps, auth, permissions, rate, wrap, cache_ttl, cache_watch })
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Code generation
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Translate `:id` style path params to `{id}` for ntex.
fn translate_path(path: &str) -> String {
    let mut result = String::with_capacity(path.len());
    let mut chars = path.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == ':' {
            result.push('{');
            while let Some(&next) = chars.peek() {
                if next == '/' || next == '.' || next == '-' {
                    break;
                }
                result.push(chars.next().unwrap());
            }
            result.push('}');
        } else {
            result.push(ch);
        }
    }

    result
}

pub fn expand_route(attr: TokenStream, item: TokenStream) -> TokenStream {
    let route_attr = match syn::parse2::<RouteAttr>(attr) {
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

    // Translate :param → {param} for ntex
    let ntex_path = translate_path(&route_attr.path);
    let original_path = &route_attr.path;
    let method_ident = route_attr.method.as_ident();
    let method_str = route_attr.method.as_str();

    // Optional metadata
    let tag_expr = match &route_attr.tag {
        Some(t) => quote! { ::core::option::Option::Some(#t) },
        None => quote! { ::core::option::Option::None },
    };
    let desc_expr = match &route_attr.desc {
        Some(d) => quote! { ::core::option::Option::Some(#d) },
        None => quote! { ::core::option::Option::None },
    };

    // Response specs — serialize as static array of (u16, &str, Option<&str>)
    let resp_count = route_attr.resps.len();
    let resp_entries: Vec<_> = route_attr.resps.iter().map(|r| {
        let status = r.status;
        let desc = &r.description;
        let ct = match &r.content_type {
            Some(ct) => quote! { ::core::option::Option::Some(#ct) },
            None => quote! { ::core::option::Option::None },
        };
        let schema_fn = match &r.schema_type {
            Some(type_path) => {
                // If it's something like Json<User>, extract the inner generic type.
                // Or if it's just `User`, use it directly. 
                // We'll trust the user to provide a type that implements `ToSchema`.
                // For simplicity, we just use the type_path directly. If it fails, compiler error.
                // Wait! If they wrote `Json<User>`, `Json<User>` maybe doesn't implement ToSchema.
                // We will try extracting the generic if the last segment has arguments.
                let mut inner_type = quote!{ #type_path };
                if let Some(segment) = type_path.path.segments.last() {
                    if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                        if let Some(syn::GenericArgument::Type(inner)) = args.args.first() {
                            inner_type = quote!{ #inner };
                        }
                    }
                }

                quote! {
                    ::core::option::Option::Some(|__vec| {
                        <#inner_type as ::floz::utoipa::ToSchema>::schemas(__vec);
                        let __name = <#inner_type as ::floz::utoipa::ToSchema>::name().into_owned();
                        let __schema = <#inner_type as ::floz::utoipa::__dev::ComposeSchema>::compose(::std::vec![]);
                        (__name, __schema)
                    })
                }
            },
            None => quote! { ::core::option::Option::None },
        };

        quote! {
            ::floz::router::ResponseMeta {
                status: #status,
                description: #desc,
                content_type: #ct,
                schema_fn: #schema_fn,
            }
        }
    }).collect();

    // Generate a unique static name for this route's registrar and response array
    let register_fn_name = syn::Ident::new(
        &format!("__floz_register_{}", fn_name),
        fn_name.span(),
    );
    let resps_static_name = syn::Ident::new(
        &format!("__FLOZ_RESPS_{}", fn_name.to_string().to_uppercase()),
        fn_name.span(),
    );

    // Auth and rate metadata
    let auth_expr = match &route_attr.auth {
        Some(a) => quote! { ::core::option::Option::Some(#a) },
        None => quote! { ::core::option::Option::None },
    };
    let perms_expr = if route_attr.permissions.is_empty() {
        quote! { ::core::option::Option::None }
    } else {
        let p = &route_attr.permissions;
        quote! { ::core::option::Option::Some(&[#(#p),*]) }
    };

    let rate_expr = match &route_attr.rate {
        Some(r) => quote! { ::core::option::Option::Some(#r) },
        None => quote! { ::core::option::Option::None },
    };

    // Cache metadata
    let cache_ttl_expr = match &route_attr.cache_ttl {
        Some(ttl) => quote! { ::core::option::Option::Some(#ttl) },
        None => quote! { ::core::option::Option::None },
    };
    
    let cache_watch_expr = if route_attr.cache_watch.is_empty() {
        quote! { ::core::option::Option::None }
    } else {
        let tags = &route_attr.cache_watch;
        quote! { ::core::option::Option::Some(&[#(#tags),*]) }
    };

    let wrap_calls = route_attr.wrap.iter().map(|w| {
        quote! { .middleware(#w) }
    });

    let expanded = quote! {
        // The handler function — use `state: State` for shared state access
        #(#fn_attrs)*
        #fn_vis #fn_sig #fn_block

        // Static response metadata array
        #[allow(non_upper_case_globals)]
        static #resps_static_name: [::floz::router::ResponseMeta; #resp_count] = [
            #(#resp_entries),*
        ];

        // Auto-register this route natively via inventory, giving us complete
        // control to inject `.middleware()` calls.
        fn #register_fn_name(cfg: &mut ::floz::ntex::web::ServiceConfig) {
            let route = ::floz::ntex::web::resource(#ntex_path)
                #(#wrap_calls)*
                .route(::floz::ntex::web::#method_ident().to(#fn_name));
            
            cfg.service(route);
        }

        ::floz::inventory::submit! {
            ::floz::router::RouteEntry::new(
                #method_str,
                #original_path,
                #tag_expr,
                #desc_expr,
                #register_fn_name,
                &#resps_static_name,
                #auth_expr,
                #perms_expr,
                #rate_expr,
                ::core::option::Option::None,
                false,
                false,
                #cache_ttl_expr,
                #cache_watch_expr,
            )
        }
    };

    expanded
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// #[floz::main] Macro
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
pub fn expand_main(item: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    let input = match syn::parse2::<syn::ItemFn>(item) {
        Ok(it) => it,
        Err(e) => return e.to_compile_error(),
    };

    // We delegate to ntex::main which sets up the ntex System registry
    quote::quote! {
        #[::floz::ntex::main]
        #input
    }
}


