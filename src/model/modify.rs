use proc_macro2::TokenStream;
use quote::quote;
use syn::{Fields, ItemStruct};
use super::col::{extract_col_name, parse_col_attrs};

/// Generate the modified struct: strip `#[col(...)]` attrs, add derives + `_dirty_flags`.
pub(crate) fn generate_modified_struct(input: &ItemStruct, soft_delete: bool) -> TokenStream {
    let vis = &input.vis;
    let name = &input.ident;

    // Preserve non-col, non-hooks attributes (doc comments, etc.)
    let struct_attrs: Vec<_> = input
        .attrs
        .iter()
        .filter(|a| !a.path().is_ident("hooks"))
        .collect();

    // Rebuild fields without #[col(...)] attributes
    let fields = match &input.fields {
        Fields::Named(named) => &named.named,
        _ => unreachable!(), // validated earlier
    };

    let clean_fields: Vec<TokenStream> = fields
        .iter()
        .map(|field| {
            let vis = &field.vis;
            let ident = &field.ident;
            let ty = &field.ty;

            // Keep non-macro attributes (doc comments, serde, etc.)
            let kept_attrs: Vec<_> = field
                .attrs
                .iter()
                .filter(|a| !a.path().is_ident("col") && !a.path().is_ident("rel") && !a.path().is_ident("m2m"))
                .collect();

            // Check if this field is a relationship
            let is_rel = field.attrs.iter().any(|a| a.path().is_ident("rel") || a.path().is_ident("m2m"));

            let rel_skip = if is_rel {
                quote! {
                    #[sqlx(skip)]
                    #[serde(default)]
                }
            } else {
                quote! {}
            };

            // Check if the rust field name differs from the DB column name
            // and add #[sqlx(rename = "...")] if needed
            let col_name = {
                let result = parse_col_attrs(field);
                let mods = match result {
                    Ok(r) => r.modifiers,
                    Err(_) => Vec::new(),
                };
                extract_col_name(&mods)
            };

            let rename_attr = if let Some(ref db_name) = col_name {
                let field_name = ident.as_ref().map(|i| i.to_string()).unwrap_or_default();
                if *db_name != field_name {
                    quote! { #[sqlx(rename = #db_name)] }
                } else {
                    quote! {}
                }
            } else {
                quote! {}
            };

            quote! {
                #(#kept_attrs)*
                #rename_attr
                #rel_skip
                #vis #ident: #ty,
            }
        })
        .collect();

    let user_has_deleted_at = clean_fields.iter().any(|ts| ts.to_string().contains("deleted_at"));
    
    let injected_deleted_at = if soft_delete && !user_has_deleted_at {
        quote! {
            pub deleted_at: ::core::option::Option<::floz::chrono::DateTime<::floz::chrono::Utc>>,
        }
    } else {
        quote! {}
    };

    quote! {
        #(#struct_attrs)*
        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
        #[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
        #vis struct #name {
            #(#clean_fields)*
            #injected_deleted_at

            /// Dirty tracking bitmask — each bit corresponds to a column.
            #[serde(skip)]
            #[sqlx(skip)]
            pub _dirty_flags: u64,
        }
    }
}
