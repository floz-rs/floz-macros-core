use super::types::type_tokens;
use crate::ast::ModelDef;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

pub fn generate_setters(model: &ModelDef) -> TokenStream {
    let name = &model.name;

    let setters: Vec<TokenStream> = model
        .db_columns
        .iter()
        .enumerate()
        .map(|(idx, field)| {
            let setter_name = format_ident!("set_{}", field.rust_name);
            let field_name = &field.rust_name;
            let rust_type = type_tokens(&field.type_info, field.is_nullable(), field.is_tz());
            let bit: u64 = 1u64 << idx;

            quote! {
                /// Set the value and mark the column as dirty.
                pub fn #setter_name(&mut self, val: #rust_type) {
                    self.#field_name = val;
                    self._dirty_flags |= #bit;
                }
            }
        })
        .collect();

    // is_dirty() helper
    let dirty_check = quote! {
        /// Returns true if any field has been modified.
        pub fn is_dirty(&self) -> bool {
            self._dirty_flags != 0
        }

        /// Returns true if a specific column (by bit index) is dirty.
        pub fn is_field_dirty(&self, bit: usize) -> bool {
            self._dirty_flags & (1u64 << bit) != 0
        }

        /// Reset dirty flags (called after save).
        pub fn clear_dirty(&mut self) {
            self._dirty_flags = 0;
        }
    };

    quote! {
        impl #name {
            #(#setters)*
            #dirty_check
        }
    }
}
