use crate::ast::TypeInfo;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

/// Convert a TypeInfo to a Rust type TokenStream.
pub fn type_tokens(type_info: &TypeInfo, nullable: bool, tz: bool) -> TokenStream {
    let base = match type_info {
        TypeInfo::Integer => quote! { i32 },
        TypeInfo::Short => quote! { i16 },
        TypeInfo::BigInt => quote! { i64 },
        TypeInfo::Real => quote! { f32 },
        TypeInfo::Double => quote! { f64 },
        TypeInfo::Decimal { .. } => quote! { sqlx::types::BigDecimal },
        TypeInfo::Varchar { .. } | TypeInfo::Text => quote! { String },
        TypeInfo::Bool => quote! { bool },
        TypeInfo::Date => quote! { chrono::NaiveDate },
        TypeInfo::Time => quote! { chrono::NaiveTime },
        TypeInfo::DateTime => {
            if tz {
                quote! { chrono::DateTime<chrono::Utc> }
            } else {
                quote! { chrono::NaiveDateTime }
            }
        }
        TypeInfo::Uuid => quote! { uuid::Uuid },
        TypeInfo::Binary => quote! { Vec<u8> },
        TypeInfo::Col { rust_type } => {
            let ty = format_ident!("{}", rust_type);
            quote! { #ty }
        }
        TypeInfo::Json | TypeInfo::Jsonb => quote! { serde_json::Value },
        TypeInfo::Ltree => quote! { String },
        TypeInfo::Enum { rust_type } => {
            let ty = format_ident!("{}", rust_type);
            quote! { #ty }
        }
        // Native PG arrays
        TypeInfo::TextArray | TypeInfo::VarcharArray => quote! { Vec<String> },
        TypeInfo::IntArray => quote! { Vec<i32> },
        TypeInfo::ShortArray => quote! { Vec<i16> },
        TypeInfo::BigIntArray => quote! { Vec<i64> },
        TypeInfo::UuidArray => quote! { Vec<uuid::Uuid> },
        TypeInfo::BoolArray => quote! { Vec<bool> },
        TypeInfo::RealArray => quote! { Vec<f32> },
        TypeInfo::DoubleArray => quote! { Vec<f64> },
    };

    if nullable {
        quote! { Option<#base> }
    } else {
        base
    }
}

/// Generate a default value TokenStream for a given type.
pub fn default_value_tokens(type_info: &TypeInfo, nullable: bool, tz: bool) -> TokenStream {
    if nullable {
        return quote! { None };
    }

    match type_info {
        TypeInfo::Integer => quote! { 0i32 },
        TypeInfo::Short => quote! { 0i16 },
        TypeInfo::BigInt => quote! { 0i64 },
        TypeInfo::Real => quote! { 0.0f32 },
        TypeInfo::Double => quote! { 0.0f64 },
        TypeInfo::Decimal { .. } => quote! { Default::default() },
        TypeInfo::Varchar { .. } | TypeInfo::Text => quote! { String::new() },
        TypeInfo::Bool => quote! { false },
        TypeInfo::Date => {
            quote! { chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap() }
        }
        TypeInfo::Time => {
            quote! { chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap() }
        }
        TypeInfo::DateTime => {
            if tz {
                quote! {
                    chrono::DateTime::from_timestamp(0, 0).unwrap()
                }
            } else {
                quote! {
                    chrono::NaiveDate::from_ymd_opt(1970, 1, 1)
                        .unwrap()
                        .and_hms_opt(0, 0, 0)
                        .unwrap()
                }
            }
        }
        TypeInfo::Uuid => quote! { uuid::Uuid::nil() },
        TypeInfo::Binary => quote! { Vec::new() },
        TypeInfo::Col { .. } | TypeInfo::Enum { .. } => quote! { Default::default() },
        TypeInfo::Json | TypeInfo::Jsonb => quote! { serde_json::Value::Null },
        TypeInfo::Ltree => quote! { String::new() },
        // Arrays
        TypeInfo::TextArray
        | TypeInfo::VarcharArray
        | TypeInfo::IntArray
        | TypeInfo::ShortArray
        | TypeInfo::BigIntArray
        | TypeInfo::UuidArray
        | TypeInfo::BoolArray
        | TypeInfo::RealArray
        | TypeInfo::DoubleArray => {
            quote! { Vec::new() }
        }
    }
}
