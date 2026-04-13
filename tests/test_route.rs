use floz_macros_core::route::{expand_route, HttpMethod, ResponseSpec, RouteAttr};

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

#[test]
fn test_translate_path_simple() {
    assert_eq!(translate_path("/users"), "/users");
}

#[test]
fn test_translate_path_single_param() {
    assert_eq!(translate_path("/users/:id"), "/users/{id}");
}

#[test]
fn test_translate_path_multiple_params() {
    assert_eq!(
        translate_path("/posts/:post_id/comments/:comment_id"),
        "/posts/{post_id}/comments/{comment_id}"
    );
}

#[test]
fn test_translate_path_no_params() {
    assert_eq!(translate_path("/health"), "/health");
}

#[test]
fn test_parse_response_spec() {
    let ts: proc_macro2::TokenStream = quote::quote! { (200, "Success", "application/json") };
    let spec: ResponseSpec = syn::parse2(ts).unwrap();
    assert_eq!(spec.status, 200);
    assert_eq!(spec.description, "Success");
    assert_eq!(spec.content_type.unwrap(), "application/json");
    assert!(spec.schema_type.is_none());

    let ts2: proc_macro2::TokenStream = quote::quote! { (404, "Not Found") };
    let spec2: ResponseSpec = syn::parse2(ts2).unwrap();
    assert_eq!(spec2.status, 404);
    assert_eq!(spec2.description, "Not Found");
    assert!(spec2.content_type.is_none());
    assert!(spec2.schema_type.is_none());

    let ts3: proc_macro2::TokenStream = quote::quote! { (201, "Created", Json<User>) };
    let spec3: ResponseSpec = syn::parse2(ts3).unwrap();
    assert_eq!(spec3.status, 201);
    assert_eq!(spec3.description, "Created");
    assert!(spec3.content_type.is_none());
    assert!(spec3.schema_type.is_some());
}

#[test]
fn test_parse_route_attr() {
    let ts: proc_macro2::TokenStream = quote::quote! {
        get: "/users/:id",
        tag: "Users",
        desc: "Get user",
        resps: [
            (200, "found")
        ],
        auth: jwt,
        rate: "10/m"
    };
    let route: RouteAttr = syn::parse2(ts).unwrap();
    assert!(matches!(route.method, HttpMethod::Get));
    assert_eq!(route.path, "/users/:id");
    assert_eq!(route.tag.unwrap(), "Users");
    assert_eq!(route.desc.unwrap(), "Get user");
    assert_eq!(route.resps.len(), 1);
    assert_eq!(route.auth.unwrap(), "jwt");
    assert_eq!(route.rate.unwrap(), "10/m");
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Cache attribute parsing tests
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn test_parse_cache_ttl_only() {
    let ts: proc_macro2::TokenStream = quote::quote! {
        get: "/health",
        cache(ttl = 30)
    };
    let route: RouteAttr = syn::parse2(ts).unwrap();
    assert_eq!(route.cache_ttl, Some(30));
    assert!(route.cache_watch.is_empty());
}

#[test]
fn test_parse_cache_ttl_and_watch() {
    let ts: proc_macro2::TokenStream = quote::quote! {
        get: "/users",
        tag: "Users",
        cache(ttl = 300, watch = ["users"])
    };
    let route: RouteAttr = syn::parse2(ts).unwrap();
    assert_eq!(route.cache_ttl, Some(300));
    assert_eq!(route.cache_watch, vec!["users"]);
}

#[test]
fn test_parse_cache_multiple_watch_tags() {
    let ts: proc_macro2::TokenStream = quote::quote! {
        get: "/users/:id",
        cache(ttl = 600, watch = ["users", "users:{id}"])
    };
    let route: RouteAttr = syn::parse2(ts).unwrap();
    assert_eq!(route.cache_ttl, Some(600));
    assert_eq!(route.cache_watch, vec!["users", "users:{id}"]);
}

#[test]
fn test_parse_cache_with_all_attrs() {
    let ts: proc_macro2::TokenStream = quote::quote! {
        get: "/users/cached",
        tag: "Users",
        desc: "List all users (cached)",
        resps: [(200, "OK")],
        cache(ttl = 300, watch = ["users"]),
        auth: jwt
    };
    let route: RouteAttr = syn::parse2(ts).unwrap();
    assert!(matches!(route.method, HttpMethod::Get));
    assert_eq!(route.path, "/users/cached");
    assert_eq!(route.tag.unwrap(), "Users");
    assert_eq!(route.desc.unwrap(), "List all users (cached)");
    assert_eq!(route.resps.len(), 1);
    assert_eq!(route.cache_ttl, Some(300));
    assert_eq!(route.cache_watch, vec!["users"]);
    assert_eq!(route.auth.unwrap(), "jwt");
}

#[test]
fn test_parse_no_cache_has_none() {
    let ts: proc_macro2::TokenStream = quote::quote! {
        post: "/users",
        tag: "Users"
    };
    let route: RouteAttr = syn::parse2(ts).unwrap();
    assert!(route.cache_ttl.is_none());
    assert!(route.cache_watch.is_empty());
}

#[test]
fn test_parse_cache_watch_only_fails() {
    // cache with only watch and no ttl should still parse
    // (ttl remains None — the middleware simply won't cache)
    let ts: proc_macro2::TokenStream = quote::quote! {
        get: "/users",
        cache(watch = ["users"])
    };
    let route: RouteAttr = syn::parse2(ts).unwrap();
    assert!(route.cache_ttl.is_none());
    assert_eq!(route.cache_watch, vec!["users"]);
}
