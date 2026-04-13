//! Code generation — transforms the parsed AST into Rust code.
//!
//! For each model, generates:
//! 1. A DAO struct with derives (Debug, Clone, Serialize, Deserialize, FromRow)
//! 2. A Table namespace struct with typed Column constants
//! 3. A Default impl for testing/mocking

use proc_macro2::TokenStream;
use quote::quote;

use crate::ast::*;

mod dao;
mod ddl;
mod defaults;
mod relations;
mod setters;
mod structs;
mod table;
mod types;
mod utils;
mod validate;

pub use relations::{derive_fk_name, derive_target_fk_name};
pub use types::{default_value_tokens, type_tokens};
pub use utils::path_to_table_ident;

fn generate_model(model: &ModelDef) -> TokenStream {
    let struct_tokens = structs::generate_struct(model);
    let impl_tokens = generate_model_impls(model);

    quote! {
        #struct_tokens
        #impl_tokens
    }
}

pub fn generate_model_impls(model: &ModelDef) -> TokenStream {
    let table_tokens = table::generate_table(model);
    let default_tokens = defaults::generate_default(model);
    let setters_tokens = setters::generate_setters(model);
    let dao_tokens = dao::generate_dao(model);
    let ddl_tokens = ddl::generate_ddl(model);
    let hooks_tokens = relations::generate_hooks(model);
    let rel_methods_tokens = relations::generate_rel_methods(model);
    let validate_tokens = validate::generate_validate(model);

    quote! {
        #table_tokens
        #default_tokens
        #setters_tokens
        #dao_tokens
        #ddl_tokens
        #hooks_tokens
        #rel_methods_tokens
        #validate_tokens
    }
}
