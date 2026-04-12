//! Parser for the `#[model("table")]` attribute macro.
//!
//! Converts a user-written Rust struct with `#[col(...)]` field attributes
//! into a `ModelDef` AST — the same representation used by `schema!` —
//! then generates all ORM code using the shared codegen.
//!
//! When `crud` is specified — `#[model("table", crud)]` — auto-generates
//! 5 REST CRUD route handlers via the `crud` module.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse2, ItemStruct};

use crate::codegen;
use crate::crud;

pub(crate) mod attr;
pub(crate) mod builder;
pub mod snapshot;
mod col;
mod modify;
mod rel;
mod types;

use attr::parse_model_attr;
use builder::build_model_def;
use modify::generate_modified_struct;

/// Entry point called by the `#[model("table_name")]` attribute macro.
pub fn expand_model(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse the table name + optional crud config from the attribute
    let (table_name, crud_config, soft_delete) = match parse_model_attr(attr) {
        Ok(parsed) => parsed,
        Err(e) => return e.to_compile_error(),
    };

    // Parse the struct definition
    let input_struct = match parse2::<ItemStruct>(item) {
        Ok(s) => s,
        Err(e) => return e.to_compile_error(),
    };

    // Build a ModelDef from the struct + attributes
    let model_def = match build_model_def(&input_struct, &table_name, soft_delete) {
        Ok(m) => m,
        Err(e) => return e.to_compile_error(),
    };

    // Generate the modified struct (with derives + _dirty_flags)
    let modified_struct = generate_modified_struct(&input_struct, soft_delete);

    // Generate all impl blocks using existing codegen
    let impl_tokens = codegen::generate_model_impls(&model_def);

    // Generate CRUD routes if requested
    let crud_tokens = if let Some(ref config) = crud_config {
        crud::generate_crud_routes(&model_def, config)
    } else {
        quote! {}
    };

    quote! {
        #modified_struct
        #impl_tokens
        #crud_tokens
    }
}
