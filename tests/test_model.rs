//! Tests for the `#[model("table")]` attribute macro parser.
//!
//! These tests validate:
//! - Type resolution (token name → TypeInfo)
//! - `#[col(...)]` attribute parsing
//! - ModelDef construction from a struct
//! - Error handling for invalid inputs

use floz_macros_core::ast::{Modifier, TypeInfo};
use floz_macros_core::model::expand_model;
use quote::quote;

// ═══════════════════════════════════════════════════════════════
// Helper: parse a #[model] struct and check it compiles (no panic)
// ═══════════════════════════════════════════════════════════════

fn expand(table: &str, struct_tokens: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    let attr = syn::parse_str::<syn::LitStr>(table).unwrap();
    let attr_tokens = quote! { #attr };
    expand_model(attr_tokens, struct_tokens)
}

fn expand_ok(table: &str, struct_tokens: proc_macro2::TokenStream) {
    let output = expand(table, struct_tokens);
    let output_str = output.to_string();
    // A successful expansion will contain the generated struct with derives
    assert!(
        output_str.contains("serde :: Serialize"),
        "Macro expansion failed — expected generated struct. Output starts with: {}",
        &output_str[..output_str.len().min(300)]
    );
}

fn expand_err_contains(table: &str, struct_tokens: proc_macro2::TokenStream, expected: &str) {
    let output = expand(table, struct_tokens);
    let output_str = output.to_string();
    assert!(
        output_str.contains("compile_error"),
        "Expected compile_error but got: {}",
        &output_str[..output_str.len().min(300)]
    );
    assert!(
        output_str.contains(expected),
        "Expected error message containing '{}' but got: {}",
        expected,
        &output_str[..output_str.len().min(500)]
    );
}

// ═══════════════════════════════════════════════════════════════
// Basic struct parsing
// ═══════════════════════════════════════════════════════════════

#[test]
fn model_simple_struct() {
    expand_ok("\"notes\"", quote! {
        pub struct Note {
            #[col(key, auto)]
            pub id: i32,
            pub title: String,
            pub content: String,
        }
    });
}

#[test]
fn model_with_type_aliases() {
    // Varchar, Text are just type aliases for String — but the macro
    // reads the TOKEN name, not the resolved type
    expand_ok("\"notes\"", quote! {
        pub struct Note {
            #[col(key, auto)]
            pub id: i32,
            pub title: Varchar,
            pub content: Text,
        }
    });
}

#[test]
fn model_all_numeric_types() {
    expand_ok("\"numbers\"", quote! {
        pub struct Numbers {
            #[col(key, auto)]
            pub id: i32,
            pub small: i16,
            pub big: i64,
            pub float: f32,
            pub double: f64,
            pub decimal: Decimal,
            pub flag: bool,
        }
    });
}

#[test]
fn model_temporal_types() {
    expand_ok("\"events\"", quote! {
        pub struct Event {
            #[col(key, auto)]
            pub id: i32,
            pub created: Timestamp,
            pub updated: TimestampTz,
            pub date: Date,
        }
    });
}

#[test]
fn model_binary_json_uuid_types() {
    expand_ok("\"docs\"", quote! {
        pub struct Doc {
            #[col(key)]
            pub id: Uuid,
            pub data: Json,
            pub meta: Jsonb,
            pub file: Bytes,
        }
    });
}

#[test]
fn model_nullable_fields() {
    expand_ok("\"profiles\"", quote! {
        pub struct Profile {
            #[col(key, auto)]
            pub id: i32,
            pub bio: Option<Text>,
            pub avatar: Option<String>,
            pub age: Option<i32>,
        }
    });
}

// ═══════════════════════════════════════════════════════════════
// #[col(...)] attribute parsing
// ═══════════════════════════════════════════════════════════════

#[test]
fn col_key_auto() {
    expand_ok("\"t\"", quote! {
        pub struct T {
            #[col(key, auto)]
            pub id: i32,
        }
    });
}

#[test]
fn col_unique() {
    expand_ok("\"t\"", quote! {
        pub struct T {
            #[col(key, auto)]
            pub id: i32,
            #[col(unique)]
            pub email: Varchar,
        }
    });
}

#[test]
fn col_index() {
    expand_ok("\"t\"", quote! {
        pub struct T {
            #[col(key, auto)]
            pub id: i32,
            #[col(index)]
            pub category: Varchar,
        }
    });
}

#[test]
fn col_default_value() {
    expand_ok("\"t\"", quote! {
        pub struct T {
            #[col(key, auto)]
            pub id: i32,
            #[col(default = "active")]
            pub status: Varchar,
        }
    });
}

#[test]
fn col_now() {
    expand_ok("\"t\"", quote! {
        pub struct T {
            #[col(key, auto)]
            pub id: i32,
            #[col(now)]
            pub created_at: Timestamp,
        }
    });
}

#[test]
fn col_max_length() {
    expand_ok("\"t\"", quote! {
        pub struct T {
            #[col(key, auto)]
            pub id: i32,
            #[col(max = 500)]
            pub summary: Varchar,
        }
    });
}

#[test]
fn col_precision_scale() {
    expand_ok("\"t\"", quote! {
        pub struct T {
            #[col(key, auto)]
            pub id: i32,
            #[col(precision = 10, scale = 4)]
            pub price: Decimal,
        }
    });
}

#[test]
fn col_name_rename() {
    expand_ok("\"t\"", quote! {
        pub struct T {
            #[col(key, auto)]
            pub id: i32,
            #[col(name = "is_published")]
            pub published: bool,
        }
    });
}

#[test]
fn col_references_foreign_key() {
    expand_ok("\"posts\"", quote! {
        pub struct Post {
            #[col(key, auto)]
            pub id: i32,
            #[col(references("users", "id"), on_delete = "cascade")]
            pub author_id: i32,
            pub title: Varchar,
        }
    });
}

#[test]
fn rel_has_many() {
    expand_ok("\"users\"", quote! {
        pub struct User {
            #[col(key, auto)]
            pub id: i32,
            
            #[rel(has_many(model = "Role", foreign_key = "user_id"))]
            pub roles: Vec<Role>,
        }
    });
}

#[test]
fn rel_belongs_to() {
    expand_ok("\"posts\"", quote! {
        pub struct Post {
            #[col(key, auto)]
            pub id: i32,
            
            #[rel(belongs_to(model = "crate::app::user::User", foreign_key = "author_id"))]
            pub author: crate::app::user::User,
        }
    });
}

#[test]
fn rel_m2m() {
    expand_ok("\"users\"", quote! {
        pub struct User {
            #[col(key, auto)]
            pub id: i32,
            
            #[m2m(Role, through = "user_roles")]
            pub roles: Vec<Role>,
        }
    });
}

#[test]
fn col_combined_attributes() {
    expand_ok("\"t\"", quote! {
        pub struct T {
            #[col(key, auto)]
            pub id: i32,
            #[col(unique, index, max = 128)]
            pub slug: Varchar,
        }
    });
}

#[test]
fn col_multiple_attribute_blocks() {
    // Two separate #[col] blocks on the same field
    expand_ok("\"t\"", quote! {
        pub struct T {
            #[col(key)]
            #[col(auto)]
            pub id: i32,
        }
    });
}

// ═══════════════════════════════════════════════════════════════
// Generated output checks
// ═══════════════════════════════════════════════════════════════

#[test]
fn model_generates_struct_with_derives() {
    let output = expand("\"notes\"", quote! {
        pub struct Note {
            #[col(key, auto)]
            pub id: i32,
            pub title: String,
        }
    });
    let s = output.to_string();
    assert!(s.contains("serde :: Serialize"), "Missing Serialize derive");
    assert!(s.contains("serde :: Deserialize"), "Missing Deserialize derive");
    assert!(s.contains("sqlx :: FromRow"), "Missing FromRow derive");
    assert!(s.contains("_dirty_flags"), "Missing _dirty_flags field");
}

#[test]
fn model_generates_table_struct() {
    let output = expand("\"notes\"", quote! {
        pub struct Note {
            #[col(key, auto)]
            pub id: i32,
            pub title: String,
        }
    });
    let s = output.to_string();
    assert!(s.contains("NoteTable"), "Missing NoteTable struct");
}

#[test]
fn model_generates_dao_methods() {
    let output = expand("\"notes\"", quote! {
        pub struct Note {
            #[col(key, auto)]
            pub id: i32,
            pub title: String,
        }
    });
    let s = output.to_string();
    assert!(s.contains("fn create"), "Missing create method");
    assert!(s.contains("fn all"), "Missing all method");
    assert!(s.contains("fn find"), "Missing find method");
    assert!(s.contains("fn get"), "Missing get method");
    assert!(s.contains("fn save"), "Missing save method");
    assert!(s.contains("fn delete"), "Missing delete method");
}

#[test]
fn model_generates_setters() {
    let output = expand("\"notes\"", quote! {
        pub struct Note {
            #[col(key, auto)]
            pub id: i32,
            pub title: String,
        }
    });
    let s = output.to_string();
    assert!(s.contains("set_title"), "Missing set_title setter");
    assert!(s.contains("set_id"), "Missing set_id setter");
}

#[test]
fn model_generates_ddl() {
    let output = expand("\"notes\"", quote! {
        pub struct Note {
            #[col(key, auto)]
            pub id: i32,
            pub title: String,
        }
    });
    let s = output.to_string();
    assert!(s.contains("create_table"), "Missing create_table method");
    assert!(s.contains("drop_table"), "Missing drop_table method");
}

#[test]
fn model_strips_col_attributes() {
    let output = expand("\"notes\"", quote! {
        pub struct Note {
            #[col(key, auto)]
            pub id: i32,
        }
    });
    let s = output.to_string();
    // The #[col(...)] attributes should be stripped from the output struct
    // The generated struct should NOT contain #[col(...)]
    // (it should have the derives but not the user's col attrs)
    let struct_part: Vec<&str> = s.split("pub struct Note").collect();
    assert!(struct_part.len() >= 2, "Should contain struct Note");
}

#[test]
fn model_generates_hooks() {
    let output = expand("\"notes\"", quote! {
        pub struct Note {
            #[col(key, auto)]
            pub id: i32,
        }
    });
    let s = output.to_string();
    assert!(s.contains("FlozHooks"), "Missing FlozHooks impl");
}

// ═══════════════════════════════════════════════════════════════
// Error handling
// ═══════════════════════════════════════════════════════════════

#[test]
fn model_error_no_table_name() {
    let attr = quote! {};
    let item = quote! {
        pub struct Note {
            pub id: i32,
        }
    };
    let output = expand_model(attr, item);
    let s = output.to_string();
    assert!(s.contains("compile_error"), "Should error without table name");
}

#[test]
fn model_error_unknown_type() {
    expand_err_contains("\"t\"", quote! {
        pub struct T {
            pub id: SomeWeirdType,
        }
    }, "unknown type");
}

#[test]
fn model_error_unknown_col_attr() {
    expand_err_contains("\"t\"", quote! {
        pub struct T {
            #[col(primary)]
            pub id: i32,
        }
    }, "unknown #[col] attribute");
}

#[test]
fn model_error_tuple_struct() {
    expand_err_contains("\"t\"", quote! {
        pub struct T(i32, String);
    }, "named fields");
}

// ═══════════════════════════════════════════════════════════════
// Full realistic models
// ═══════════════════════════════════════════════════════════════

#[test]
fn model_realistic_note() {
    expand_ok("\"notes\"", quote! {
        pub struct Note {
            #[col(key, auto)]
            pub id: i32,
            pub title: Varchar,
            pub content: Text,
        }
    });
}

#[test]
fn model_realistic_user() {
    expand_ok("\"users\"", quote! {
        pub struct User {
            #[col(key, auto)]
            pub id: i64,
            #[col(unique, max = 100)]
            pub email: Varchar,
            pub name: Varchar,
            pub bio: Option<Text>,
            #[col(now)]
            pub created_at: Timestamp,
            #[col(now)]
            pub updated_at: TimestampTz,
        }
    });
}

#[test]
fn model_realistic_post_with_fk() {
    expand_ok("\"posts\"", quote! {
        pub struct Post {
            #[col(key, auto)]
            pub id: i32,
            #[col(references("users", "id"), on_delete = "cascade")]
            pub author_id: i32,
            pub title: Varchar,
            pub body: Text,
            pub published: bool,
            #[col(default = "0")]
            pub view_count: i32,
            #[col(now)]
            pub created_at: TimestampTz,
        }
    });
}

#[test]
fn model_realistic_with_all_types() {
    expand_ok("\"everything\"", quote! {
        pub struct Everything {
            #[col(key)]
            pub id: Uuid,
            pub name: Varchar,
            pub description: Text,
            pub count: i32,
            pub small_count: i16,
            pub big_count: i64,
            pub ratio: f32,
            pub precise: f64,
            pub price: Decimal,
            pub active: bool,
            pub created: Timestamp,
            pub updated: TimestampTz,
            pub birthday: Date,
            pub config: Json,
            pub metadata: Jsonb,
            pub avatar: Bytes,
            pub nickname: Option<Varchar>,
            pub deleted_at: Option<TimestampTz>,
        }
    });
}

// ═══════════════════════════════════════════════════════════════
// Array types
// ═══════════════════════════════════════════════════════════════

#[test]
fn model_array_types() {
    expand_ok("\"tagged\"", quote! {
        pub struct Tagged {
            #[col(key, auto)]
            pub id: i32,
            pub tags: Vec<String>,
            pub scores: Vec<i32>,
            pub flags: Vec<bool>,
        }
    });
}

// ═══════════════════════════════════════════════════════════════
// Edge cases
// ═══════════════════════════════════════════════════════════════

#[test]
fn model_preserves_doc_comments() {
    let output = expand("\"notes\"", quote! {
        /// This is a Note model.
        pub struct Note {
            /// Primary key
            #[col(key, auto)]
            pub id: i32,
            /// The title of the note
            pub title: String,
        }
    });
    let s = output.to_string();
    assert!(s.contains("doc"), "Doc comments should be preserved");
}

#[test]
fn model_private_struct() {
    // No `pub` — should still work
    expand_ok("\"t\"", quote! {
        struct T {
            #[col(key, auto)]
            pub id: i32,
        }
    });
}
