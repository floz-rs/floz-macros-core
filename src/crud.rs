//! `crud` option for `#[model]` — auto-generates REST CRUD routes.
//!
//! When `#[model("notes", crud)]` is used, this module generates 5 route
//! handlers (list, create, get, update, delete) and auto-registers them
//! via `inventory::submit!`, identical to what `#[route]` produces.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::ast::*;

// ═══════════════════════════════════════════════════════════════
// CrudConfig — parsed from #[model("table", crud(...))]
// ═══════════════════════════════════════════════════════════════

/// Which CRUD operations to generate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CrudOp {
    List,
    Create,
    Get,
    Update,
    Delete,
}

impl CrudOp {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "list" => Some(CrudOp::List),
            "create" => Some(CrudOp::Create),
            "get" => Some(CrudOp::Get),
            "update" => Some(CrudOp::Update),
            "delete" => Some(CrudOp::Delete),
            _ => None,
        }
    }

    fn all() -> Vec<Self> {
        vec![
            CrudOp::List,
            CrudOp::Create,
            CrudOp::Get,
            CrudOp::Update,
            CrudOp::Delete,
        ]
    }
}

/// Configuration parsed from `crud(...)` options.
#[derive(Debug, Clone)]
pub struct CrudConfig {
    /// OpenAPI tag for grouping. Default: model name.
    pub tag: Option<String>,
    /// Custom path prefix. Default: /{table_name}
    pub path: Option<String>,
    /// Only generate these operations. Default: all.
    pub only: Option<Vec<CrudOp>>,
    /// Exclude these operations. Default: none.
    pub exclude: Option<Vec<CrudOp>>,
    /// Auth requirement for all routes. Default: none.
    pub auth: Option<String>,
}

impl Default for CrudConfig {
    fn default() -> Self {
        Self {
            tag: None,
            path: None,
            only: None,
            exclude: None,
            auth: None,
        }
    }
}

impl CrudConfig {
    /// Get the list of operations to generate.
    pub fn operations(&self) -> Vec<CrudOp> {
        let mut ops = if let Some(ref only) = self.only {
            only.clone()
        } else {
            CrudOp::all()
        };

        if let Some(ref exclude) = self.exclude {
            ops.retain(|op| !exclude.contains(op));
        }

        ops
    }

    /// Get the base path for routes (e.g., "/notes").
    pub fn base_path(&self, table_name: &str) -> String {
        self.path
            .clone()
            .unwrap_or_else(|| format!("/{}", table_name))
    }

    /// Get the tag for OpenAPI docs.
    pub fn tag_name(&self, model_name: &str) -> String {
        self.tag.clone().unwrap_or_else(|| model_name.to_string())
    }
}

// ═══════════════════════════════════════════════════════════════
// CrudConfig parsing from token stream
// ═══════════════════════════════════════════════════════════════

/// Parse `crud` or `crud(tag = "...", path = "...", ...)` from a ParseStream.
pub fn parse_crud_config(meta: &syn::meta::ParseNestedMeta<'_>) -> syn::Result<CrudConfig> {
    let mut config = CrudConfig::default();

    // Plain `crud` with no args
    if meta.input.is_empty() || meta.input.peek(syn::Token![,]) {
        return Ok(config);
    }

    // `crud(...)` with args
    meta.parse_nested_meta(|nested| {
        let ident = nested.path.get_ident()
            .ok_or_else(|| nested.error("expected identifier"))?;

        match ident.to_string().as_str() {
            "tag" => {
                let value = nested.value()?;
                let lit: syn::LitStr = value.parse()?;
                config.tag = Some(lit.value());
            }
            "path" => {
                let value = nested.value()?;
                let lit: syn::LitStr = value.parse()?;
                config.path = Some(lit.value());
            }
            "auth" => {
                let value = nested.value()?;
                let lit: syn::LitStr = value.parse()?;
                config.auth = Some(lit.value());
            }
            "only" => {
                let mut ops = Vec::new();
                nested.parse_nested_meta(|op_meta| {
                    let op_name = op_meta.path.get_ident()
                        .ok_or_else(|| op_meta.error("expected operation name"))?
                        .to_string();
                    match CrudOp::from_str(&op_name) {
                        Some(op) => ops.push(op),
                        None => return Err(op_meta.error(format!(
                            "unknown CRUD operation `{}`. Available: list, create, get, update, delete",
                            op_name
                        ))),
                    }
                    Ok(())
                })?;
                config.only = Some(ops);
            }
            "exclude" => {
                let mut ops = Vec::new();
                nested.parse_nested_meta(|op_meta| {
                    let op_name = op_meta.path.get_ident()
                        .ok_or_else(|| op_meta.error("expected operation name"))?
                        .to_string();
                    match CrudOp::from_str(&op_name) {
                        Some(op) => ops.push(op),
                        None => return Err(op_meta.error(format!(
                            "unknown CRUD operation `{}`. Available: list, create, get, update, delete",
                            op_name
                        ))),
                    }
                    Ok(())
                })?;
                config.exclude = Some(ops);
            }
            other => {
                return Err(nested.error(format!(
                    "unknown crud option `{}`. Available: tag, path, auth, only, exclude",
                    other
                )));
            }
        }

        Ok(())
    })?;

    Ok(config)
}

// ═══════════════════════════════════════════════════════════════
// CRUD Route Code Generation
// ═══════════════════════════════════════════════════════════════

/// Generate all CRUD route handlers + inventory registrations for a model.
pub fn generate_crud_routes(model: &ModelDef, config: &CrudConfig) -> TokenStream {
    let ops = config.operations();
    let base_path = config.base_path(&model.table_name);
    let tag = config.tag_name(&model.name.to_string());
    let model_name = &model.name;

    // Find primary key column(s)
    let pk_cols = model.primary_key_columns();
    if pk_cols.is_empty() {
        return quote! {
            compile_error!("CRUD requires a primary key. Add #[col(key)] to a field.");
        };
    }
    if pk_cols.len() > 1 {
        return quote! {
            compile_error!("CRUD does not support composite primary keys yet.");
        };
    }

    let pk = pk_cols[0];
    let pk_name = &pk.rust_name;
    let pk_type = crate::codegen::type_tokens(&pk.type_info, pk.is_nullable(), pk.is_tz());

    // The item path with {id}
    let item_path = format!("{}/{{id}}", base_path);
    // ntex path (already using {id})
    let ntex_base = &base_path;
    let ntex_item = &item_path;

    let auth_expr = match &config.auth {
        Some(a) => quote! { ::core::option::Option::Some(#a) },
        None => quote! { ::core::option::Option::None },
    };

    let model_name_lower = model.name.to_string().to_lowercase();

    let model_schema_fn = quote! {
        ::core::option::Option::Some(|__vec: &mut ::std::vec::Vec<(::std::string::String, ::floz::utoipa::openapi::RefOr<::floz::utoipa::openapi::schema::Schema>)>| {
            <#model_name as ::floz::utoipa::ToSchema>::schemas(__vec);
            let __name = <#model_name as ::floz::utoipa::ToSchema>::name().into_owned();
            let __schema = <#model_name as ::floz::utoipa::__dev::ComposeSchema>::compose(::std::vec![]);
            (__name, __schema)
        })
    };

    let list_schema_fn = quote! {
        ::core::option::Option::Some(|__vec: &mut ::std::vec::Vec<(::std::string::String, ::floz::utoipa::openapi::RefOr<::floz::utoipa::openapi::schema::Schema>)>| {
            <#model_name as ::floz::utoipa::ToSchema>::schemas(__vec);
            let __model_name = <#model_name as ::floz::utoipa::ToSchema>::name().into_owned();
            let __model_schema = <#model_name as ::floz::utoipa::__dev::ComposeSchema>::compose(::std::vec![]);
            __vec.push((__model_name.clone(), __model_schema.into()));

            let __list_name = format!("{}Paginated", __model_name);

            let __list_schema = ::floz::utoipa::openapi::schema::ObjectBuilder::new()
                .property("items", ::floz::utoipa::openapi::schema::ArrayBuilder::new()
                    .items(::floz::utoipa::openapi::schema::Ref::new(format!("#/components/schemas/{}", __model_name)))
                )
                .property("total", ::floz::utoipa::openapi::schema::ObjectBuilder::new().schema_type(::floz::utoipa::openapi::schema::Type::Integer))
                .property("page", ::floz::utoipa::openapi::schema::ObjectBuilder::new().schema_type(::floz::utoipa::openapi::schema::Type::Integer))
                .property("per_page", ::floz::utoipa::openapi::schema::ObjectBuilder::new().schema_type(::floz::utoipa::openapi::schema::Type::Integer))
                .property("total_pages", ::floz::utoipa::openapi::schema::ObjectBuilder::new().schema_type(::floz::utoipa::openapi::schema::Type::Integer))
                .property("has_next", ::floz::utoipa::openapi::schema::ObjectBuilder::new().schema_type(::floz::utoipa::openapi::schema::Type::Boolean))
                .property("has_prev", ::floz::utoipa::openapi::schema::ObjectBuilder::new().schema_type(::floz::utoipa::openapi::schema::Type::Boolean))
                .required("items")
                .required("total")
                .required("page")
                .required("per_page")
                .required("total_pages")
                .required("has_next")
                .required("has_prev")
                .build();

            ("".to_string(), ::floz::utoipa::openapi::schema::Schema::Object(__list_schema).into())
        })
    };

    let empty_register_fn = format_ident!("__crud_register_{}_empty", model_name_lower);
    let mut generated = Vec::new();

    generated.push(quote! {
        fn #empty_register_fn(_cfg: &mut ::floz::ntex::web::ServiceConfig) {}
    });

    let base_register_fn = format_ident!("__crud_register_{}_base", model_name_lower);
    let item_register_fn = format_ident!("__crud_register_{}_item", model_name_lower);

    let mut base_routes = Vec::new();
    let mut item_routes = Vec::new();

    // Collect base routes
    if ops.contains(&CrudOp::List) {
        let fn_name = format_ident!("__crud_{}_list", model_name_lower);
        base_routes.push(quote! { .route(::floz::ntex::web::get().to(#fn_name)) });
    }
    if ops.contains(&CrudOp::Create) {
        let fn_name = format_ident!("__crud_{}_create", model_name_lower);
        base_routes.push(quote! { .route(::floz::ntex::web::post().to(#fn_name)) });
    }

    if !base_routes.is_empty() {
        generated.push(quote! {
            fn #base_register_fn(cfg: &mut ::floz::ntex::web::ServiceConfig) {
                cfg.service(
                    ::floz::ntex::web::resource(#ntex_base)
                        #(#base_routes)*
                );
            }
        });
    }

    // Collect item routes
    if ops.contains(&CrudOp::Get) {
        let fn_name = format_ident!("__crud_{}_get", model_name_lower);
        item_routes.push(quote! { .route(::floz::ntex::web::get().to(#fn_name)) });
    }
    if ops.contains(&CrudOp::Update) {
        let fn_name = format_ident!("__crud_{}_update", model_name_lower);
        item_routes.push(quote! { .route(::floz::ntex::web::put().to(#fn_name)) });
    }
    if ops.contains(&CrudOp::Delete) {
        let fn_name = format_ident!("__crud_{}_delete", model_name_lower);
        item_routes.push(quote! { .route(::floz::ntex::web::delete().to(#fn_name)) });
    }

    if !item_routes.is_empty() {
        generated.push(quote! {
            fn #item_register_fn(cfg: &mut ::floz::ntex::web::ServiceConfig) {
                cfg.service(
                    ::floz::ntex::web::resource(#ntex_item)
                        #(#item_routes)*
                );
            }
        });
    }

    let mut assigned_base = false;
    let mut assigned_item = false;

    for op in &ops {
        match op {
            CrudOp::List => {
                let fn_name = format_ident!("__crud_{}_list", model_name_lower);
                let resps_name =
                    format_ident!("__CRUD_RESPS_{}_LIST", model_name_lower.to_uppercase());
                let desc = format!("List all {}", model.table_name);
                let register_fn = if !assigned_base {
                    assigned_base = true;
                    &base_register_fn
                } else {
                    &empty_register_fn
                };

                let list_preloads: Vec<TokenStream> = model.relationships.iter().map(|rel| {
                    let rel_str = rel.rust_name.to_string();
                    let preload_method = format_ident!("preload_{}", rel.rust_name);
                    quote! {
                        if let Some(preload_str) = &p.preload {
                            if preload_str.split(',').any(|s| s.trim() == #rel_str) {
                                if let Err(e) = #model_name::#preload_method(&mut page_data.items, &ctx.app.db()).await {
                                    return ::floz::ntex::web::HttpResponse::InternalServerError()
                                        .json(&::floz::serde_json::json!({"error": format!("Preload error for {}: {}", #rel_str, e)}));
                                }
                            }
                        }
                    }
                }).collect();

                generated.push(quote! {
                    #[allow(non_snake_case)]
                    async fn #fn_name(
                        ctx: ::floz::app::Context,
                        params: ::floz::ntex::web::types::Query<::floz::controller::pagination::PaginationParams>,
                    ) -> ::floz::ntex::web::HttpResponse {
                        let p = params.into_inner();
                        let page_arg = if p.limit > 0 { (p.offset / p.limit) + 1 } else { 1 };
                        
                        let query = #model_name::paginate()
                            .page(page_arg as i64)
                            .per_page(p.limit as i64);
                            
                        match query.execute(&ctx.app.db()).await {
                            Ok(mut page_data) => {
                                #(#list_preloads)*
                                ::floz::ntex::web::HttpResponse::Ok().json(&page_data)
                            },
                            Err(e) => ::floz::ntex::web::HttpResponse::InternalServerError()
                                .json(&::floz::serde_json::json!({"error": e.to_string()})),
                        }
                    }

                    #[allow(non_upper_case_globals)]
                    static #resps_name: [::floz::router::ResponseMeta; 1] = [
                        ::floz::router::ResponseMeta {
                            status: 200,
                            description: "Success",
                            content_type: ::core::option::Option::None,
                            schema_fn: #list_schema_fn,
                        },
                    ];

                    ::floz::inventory::submit! {
                        ::floz::router::RouteEntry::new(
                            "get",
                            #ntex_base,
                            ::core::option::Option::Some(#tag),
                            ::core::option::Option::Some(#desc),
                            #register_fn,
                            &#resps_name,
                            #auth_expr,
                            ::core::option::Option::None,
                            ::core::option::Option::None,
                            ::core::option::Option::None,
                            true,
                            true,
                            ::core::option::Option::None,
                            ::core::option::Option::None,
                        )
                    }
                });
            }

            CrudOp::Create => {
                let fn_name = format_ident!("__crud_{}_create", model_name_lower);
                let resps_name =
                    format_ident!("__CRUD_RESPS_{}_CREATE", model_name_lower.to_uppercase());
                let desc = format!("Create a {}", model.name);
                let register_fn = if !assigned_base {
                    assigned_base = true;
                    &base_register_fn
                } else {
                    &empty_register_fn
                };

                generated.push(quote! {
                    #[allow(non_snake_case)]
                    async fn #fn_name(
                        ctx: ::floz::app::Context,
                        body: ::floz::ntex::web::types::Json<#model_name>,
                    ) -> ::floz::ntex::web::HttpResponse {
                        let item = body.into_inner();
                        match item.create(&ctx.app.db()).await {
                            Ok(created) => ::floz::ntex::web::HttpResponse::Created().json(&created),
                            Err(e) => ::floz::ntex::web::HttpResponse::InternalServerError()
                                .json(&::floz::serde_json::json!({"error": e.to_string()})),
                        }
                    }

                    #[allow(non_upper_case_globals)]
                    static #resps_name: [::floz::router::ResponseMeta; 1] = [
                        ::floz::router::ResponseMeta {
                            status: 201,
                            description: "Created",
                            content_type: ::core::option::Option::None,
                            schema_fn: #model_schema_fn,
                        },
                    ];

                    ::floz::inventory::submit! {
                        ::floz::router::RouteEntry::new(
                            "post",
                            #ntex_base,
                            ::core::option::Option::Some(#tag),
                            ::core::option::Option::Some(#desc),
                            #register_fn,
                            &#resps_name,
                            #auth_expr,
                            ::core::option::Option::None,
                            ::core::option::Option::None,
                            #model_schema_fn,
                            false,
                            false,
                            ::core::option::Option::None,
                            ::core::option::Option::None,
                        )
                    }
                });
            }

            CrudOp::Get => {
                let fn_name = format_ident!("__crud_{}_get", model_name_lower);
                let resps_name =
                    format_ident!("__CRUD_RESPS_{}_GET", model_name_lower.to_uppercase());
                let desc = format!("Get {} by ID", model.name);
                let register_fn = if !assigned_item {
                    assigned_item = true;
                    &item_register_fn
                } else {
                    &empty_register_fn
                };

                let item_preloads: Vec<TokenStream> = model.relationships.iter().map(|rel| {
                    let rel_str = rel.rust_name.to_string();
                    let preload_method = format_ident!("preload_{}", rel.rust_name);
                    quote! {
                        if let Some(preload_str) = &p.preload {
                            if preload_str.split(',').any(|s| s.trim() == #rel_str) {
                                if let Err(e) = #model_name::#preload_method(::std::slice::from_mut(&mut item), &ctx.app.db()).await {
                                    return ::floz::ntex::web::HttpResponse::InternalServerError()
                                        .json(&::floz::serde_json::json!({"error": format!("Preload error for {}: {}", #rel_str, e)}));
                                }
                            }
                        }
                    }
                }).collect();

                generated.push(quote! {
                    #[allow(non_snake_case)]
                    async fn #fn_name(
                        ctx: ::floz::app::Context,
                        path: ::floz::ntex::web::types::Path<#pk_type>,
                        params: ::floz::ntex::web::types::Query<::floz::controller::pagination::PaginationParams>,
                    ) -> ::floz::ntex::web::HttpResponse {
                        let #pk_name = path.into_inner();
                        let p = params.into_inner();
                        match #model_name::find(#pk_name, &ctx.app.db()).await {
                            Ok(Some(mut item)) => {
                                #(#item_preloads)*
                                ::floz::ntex::web::HttpResponse::Ok().json(&item)
                            },
                            Ok(None) => ::floz::ntex::web::HttpResponse::NotFound()
                                .json(&::floz::serde_json::json!({"error": "not found"})),
                            Err(e) => ::floz::ntex::web::HttpResponse::InternalServerError()
                                .json(&::floz::serde_json::json!({"error": e.to_string()})),
                        }
                    }

                    #[allow(non_upper_case_globals)]
                    static #resps_name: [::floz::router::ResponseMeta; 2] = [
                        ::floz::router::ResponseMeta {
                            status: 200,
                            description: "Found",
                            content_type: ::core::option::Option::None,
                            schema_fn: #model_schema_fn,
                        },
                        ::floz::router::ResponseMeta {
                            status: 404,
                            description: "Not found",
                            content_type: ::core::option::Option::None,
                            schema_fn: ::core::option::Option::None,
                        },
                    ];

                    ::floz::inventory::submit! {
                        ::floz::router::RouteEntry::new(
                            "get",
                            #ntex_item,
                            ::core::option::Option::Some(#tag),
                            ::core::option::Option::Some(#desc),
                            #register_fn,
                            &#resps_name,
                            #auth_expr,
                            ::core::option::Option::None,
                            ::core::option::Option::None,
                            ::core::option::Option::None,
                            false,
                            true,
                            ::core::option::Option::None,
                            ::core::option::Option::None,
                        )
                    }
                });
            }

            CrudOp::Update => {
                let fn_name = format_ident!("__crud_{}_update", model_name_lower);
                let resps_name =
                    format_ident!("__CRUD_RESPS_{}_UPDATE", model_name_lower.to_uppercase());
                let desc = format!("Update {} by ID", model.name);
                let register_fn = if !assigned_item {
                    assigned_item = true;
                    &item_register_fn
                } else {
                    &empty_register_fn
                };

                // Generate set_* calls for each non-PK field
                let update_fields: Vec<TokenStream> = model
                    .db_columns
                    .iter()
                    .filter(|f| !f.is_primary() && !f.is_auto_increment())
                    .map(|field| {
                        let col_name = &field.column_name;
                        let setter = format_ident!("set_{}", field.rust_name);
                        quote! {
                            if let Some(val) = obj.get(#col_name) {
                                if let Ok(v) = ::floz::serde_json::from_value(val.clone()) {
                                    item.#setter(v);
                                }
                            }
                        }
                    })
                    .collect();

                generated.push(quote! {
                    #[allow(non_snake_case)]
                    async fn #fn_name(
                        ctx: ::floz::app::Context,
                        path: ::floz::ntex::web::types::Path<#pk_type>,
                        body: ::floz::ntex::web::types::Json<::floz::serde_json::Value>,
                    ) -> ::floz::ntex::web::HttpResponse {
                        let #pk_name = path.into_inner();
                        match #model_name::find(#pk_name, &ctx.app.db()).await {
                            Ok(Some(mut item)) => {
                                let updates = body.into_inner();
                                if let Some(obj) = updates.as_object() {
                                    #(#update_fields)*
                                }
                                match item.save(&ctx.app.db()).await {
                                    Ok(()) => ::floz::ntex::web::HttpResponse::Ok().json(&item),
                                    Err(e) => ::floz::ntex::web::HttpResponse::InternalServerError()
                                        .json(&::floz::serde_json::json!({"error": e.to_string()})),
                                }
                            }
                            Ok(None) => ::floz::ntex::web::HttpResponse::NotFound()
                                .json(&::floz::serde_json::json!({"error": "not found"})),
                            Err(e) => ::floz::ntex::web::HttpResponse::InternalServerError()
                                .json(&::floz::serde_json::json!({"error": e.to_string()})),
                        }
                    }

                    #[allow(non_upper_case_globals)]
                    static #resps_name: [::floz::router::ResponseMeta; 2] = [
                        ::floz::router::ResponseMeta {
                            status: 200,
                            description: "Updated",
                            content_type: ::core::option::Option::None,
                            schema_fn: #model_schema_fn,
                        },
                        ::floz::router::ResponseMeta {
                            status: 404,
                            description: "Not found",
                            content_type: ::core::option::Option::None,
                            schema_fn: ::core::option::Option::None,
                        },
                    ];

                    ::floz::inventory::submit! {
                        ::floz::router::RouteEntry::new(
                            "put",
                            #ntex_item,
                            ::core::option::Option::Some(#tag),
                            ::core::option::Option::Some(#desc),
                            #register_fn,
                            &#resps_name,
                            #auth_expr,
                            ::core::option::Option::None,
                            ::core::option::Option::None,
                            #model_schema_fn,
                            false,
                            false,
                            ::core::option::Option::None,
                            ::core::option::Option::None,
                        )
                    }
                });
            }

            CrudOp::Delete => {
                let fn_name = format_ident!("__crud_{}_delete", model_name_lower);
                let resps_name =
                    format_ident!("__CRUD_RESPS_{}_DELETE", model_name_lower.to_uppercase());
                let desc = format!("Delete {} by ID", model.name);
                let register_fn = if !assigned_item {
                    assigned_item = true;
                    &item_register_fn
                } else {
                    &empty_register_fn
                };

                generated.push(quote! {
                    #[allow(non_snake_case)]
                    async fn #fn_name(
                        ctx: ::floz::app::Context,
                        path: ::floz::ntex::web::types::Path<#pk_type>,
                    ) -> ::floz::ntex::web::HttpResponse {
                        let #pk_name = path.into_inner();
                        match #model_name::find(#pk_name, &ctx.app.db()).await {
                            Ok(Some(item)) => {
                                match item.delete(&ctx.app.db()).await {
                                    Ok(()) => ::floz::ntex::web::HttpResponse::NoContent().finish(),
                                    Err(e) => ::floz::ntex::web::HttpResponse::InternalServerError()
                                        .json(&::floz::serde_json::json!({"error": e.to_string()})),
                                }
                            }
                            Ok(None) => ::floz::ntex::web::HttpResponse::NotFound()
                                .json(&::floz::serde_json::json!({"error": "not found"})),
                            Err(e) => ::floz::ntex::web::HttpResponse::InternalServerError()
                                .json(&::floz::serde_json::json!({"error": e.to_string()})),
                        }
                    }

                    #[allow(non_upper_case_globals)]
                    static #resps_name: [::floz::router::ResponseMeta; 2] = [
                        ::floz::router::ResponseMeta {
                            status: 204,
                            description: "Deleted",
                            content_type: ::core::option::Option::None,
                            schema_fn: ::core::option::Option::None,
                        },
                        ::floz::router::ResponseMeta {
                            status: 404,
                            description: "Not found",
                            content_type: ::core::option::Option::None,
                            schema_fn: ::core::option::Option::None,
                        },
                    ];

                    ::floz::inventory::submit! {
                        ::floz::router::RouteEntry::new(
                            "delete",
                            #ntex_item,
                            ::core::option::Option::Some(#tag),
                            ::core::option::Option::Some(#desc),
                            #register_fn,
                            &#resps_name,
                            #auth_expr,
                            ::core::option::Option::None,
                            ::core::option::Option::None,
                            ::core::option::Option::None,
                            false,
                            false,
                            ::core::option::Option::None,
                            ::core::option::Option::None,
                        )
                    }
                });
            }
        }
    }

    quote! { #(#generated)* }
}
