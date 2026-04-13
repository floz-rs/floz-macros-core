use super::types::default_value_tokens;
use crate::ast::ModelDef;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

pub fn generate_default(model: &ModelDef) -> TokenStream {
    let name = &model.name;

    let default_fields: Vec<TokenStream> = model
        .db_columns
        .iter()
        .map(|field| {
            let rust_name = &field.rust_name;
            let default_val =
                default_value_tokens(&field.type_info, field.is_nullable(), field.is_tz());
            quote! { #rust_name: #default_val, }
        })
        .collect();

    let default_rels: Vec<TokenStream> = model
        .relationships
        .iter()
        .map(|rel| {
            let rel_name = format_ident!("{}", rel.rust_name);
            quote! { #rel_name: Vec::new(), }
        })
        .collect();

    quote! {
        impl Default for #name {
            fn default() -> Self {
                Self {
                    #(#default_fields)*
                    _dirty_flags: 0,
                    #(#default_rels)*
                }
            }
        }
    }
}
