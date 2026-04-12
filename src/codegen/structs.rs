use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use crate::ast::ModelDef;
use super::types::type_tokens;

pub fn generate_struct(model: &ModelDef) -> TokenStream {
    let name = &model.name;

    let fields: Vec<TokenStream> = model
        .db_columns
        .iter()
        .map(|field| {
            let rust_name = &field.rust_name;
            let rust_type = type_tokens(&field.type_info, field.is_nullable(), field.is_tz());
            let col_name = &field.column_name;

            // Add #[sqlx(rename = "...")] if rust field name != db column name
            let rename_attr = if rust_name.to_string() != *col_name {
                quote! { #[sqlx(rename = #col_name)] }
            } else {
                quote! {}
            };

            quote! {
                #rename_attr
                pub #rust_name: #rust_type,
            }
        })
        .collect();

    let rel_fields: Vec<TokenStream> = model
        .relationships
        .iter()
        .map(|rel| {
            let rel_name = format_ident!("{}", rel.rust_name);
            let target_type = &rel.target_model;
            quote! {
                /// Eager-loaded relationship items.
                #[serde(skip)]
                #[sqlx(skip)]
                pub #rel_name: Vec<#target_type>,
            }
        })
        .collect();

    quote! {
        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow, utoipa::ToSchema)]
        pub struct #name {
            #(#fields)*

            /// Dirty tracking bitmask — each bit corresponds to a column.
            /// Skipped by sqlx (not a DB column) and serde (internal state).
            #[serde(skip)]
            #[sqlx(skip)]
            pub _dirty_flags: u64,

            #(#rel_fields)*
        }
    }
}
