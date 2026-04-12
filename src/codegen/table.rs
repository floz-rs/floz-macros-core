use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use crate::ast::ModelDef;
use super::types::type_tokens;

pub fn generate_table(model: &ModelDef) -> TokenStream {
    let table_struct = format_ident!("{}Table", model.name);
    let table_name_str = &model.table_name;

    let column_consts: Vec<TokenStream> = model
        .db_columns
        .iter()
        .map(|field| {
            let const_name = &field.rust_name;
            let col_name = &field.column_name;
            let rust_type = type_tokens(&field.type_info, field.is_nullable(), field.is_tz());

            quote! {
                #[allow(non_upper_case_globals)]
                pub const #const_name: floz::Column<#rust_type> =
                    floz::Column::new(#col_name, #table_name_str);
            }
        })
        .collect();

    // Collect all column names as a const array (useful for SELECT generation)
    let col_names: Vec<&String> = model.db_columns.iter().map(|f| &f.column_name).collect();
    let col_count = model.db_columns.len();

    quote! {
        pub struct #table_struct;

        #[allow(non_upper_case_globals)]
        impl #table_struct {
            /// The PostgreSQL table name.
            pub const TABLE_NAME: &'static str = #table_name_str;

            /// All column names in declaration order.
            pub const COLUMNS: [&'static str; #col_count] = [#(#col_names),*];

            #(#column_consts)*
        }
    }
}
