use proc_macro2::TokenStream;
use quote::quote;
use crate::ast::{FieldDef, TypeInfo};
use super::types::type_tokens;

/// Quote a table name for generated SQL strings.
pub fn quote_table_str(name: &str) -> String {
    if name.contains('.') {
        name.split('.')
            .map(|p| format!("\"{}\"", p))
            .collect::<Vec<_>>()
            .join(".")
    } else {
        name.to_string()
    }
}

/// Generate function parameters, WHERE clause, and param expressions for PK queries.
/// Returns (fn_params, where_sql, param_exprs).
pub fn pk_query_parts(pk_cols: &[&FieldDef]) -> (Vec<TokenStream>, String, Vec<TokenStream>) {
    let mut fn_params = Vec::new();
    let mut where_parts = Vec::new();
    let mut param_exprs = Vec::new();

    for (i, pk) in pk_cols.iter().enumerate() {
        let param_name = &pk.rust_name;
        let param_type = type_tokens(&pk.type_info, pk.is_nullable(), pk.is_tz());
        let col_name = &pk.column_name;
        let idx = i + 1;

        fn_params.push(quote! { #param_name: #param_type });
        where_parts.push(format!("{} = ${}", col_name, idx));
        param_exprs.push(to_value_expr_ident(pk, param_name));
    }

    let where_clause = where_parts.join(" AND ");
    (fn_params, where_clause, param_exprs)
}

/// Convert a field value on `self` to a `Value` expression.
pub fn to_value_expr(field: &FieldDef, receiver: TokenStream) -> TokenStream {
    let field_name = &field.rust_name;
    to_value_tokens(&field.type_info, field.is_nullable(), field.is_tz(), quote! { #receiver.#field_name })
}

/// Convert a field value from a named parameter to a `Value` expression.
pub fn to_value_expr_ident(field: &FieldDef, ident: &proc_macro2::Ident) -> TokenStream {
    to_value_tokens(&field.type_info, field.is_nullable(), field.is_tz(), quote! { #ident })
}

/// Map a Rust field expression to the appropriate `Value` variant.
pub fn to_value_tokens(type_info: &TypeInfo, nullable: bool, tz: bool, expr: TokenStream) -> TokenStream {
    if nullable {
        return match type_info {
            TypeInfo::Integer => quote! { floz::Value::OptionInt(#expr) },
            TypeInfo::Short => quote! { floz::Value::OptionShort(#expr) },
            TypeInfo::BigInt => quote! { floz::Value::OptionBigInt(#expr) },
            TypeInfo::Real => quote! { floz::Value::OptionReal(#expr) },
            TypeInfo::Double => quote! { floz::Value::OptionDouble(#expr) },
            TypeInfo::Bool => quote! { floz::Value::OptionBool(#expr) },
            TypeInfo::Varchar { .. } | TypeInfo::Text => quote! { floz::Value::OptionString(#expr.clone()) },
            TypeInfo::Uuid => quote! { floz::Value::OptionUuid(#expr) },
            TypeInfo::DateTime => {
                if tz {
                    quote! { floz::Value::OptionDateTime(#expr) }
                } else {
                    quote! { floz::Value::OptionNaiveDateTime(#expr) }
                }
            }
            TypeInfo::Date => quote! { floz::Value::OptionNaiveDate(#expr) },
            TypeInfo::Time => quote! { floz::Value::OptionNaiveTime(#expr) },
            TypeInfo::Binary => quote! { floz::Value::OptionBytes(#expr.clone()) },
            TypeInfo::Json => quote! { floz::Value::OptionJson(#expr.clone()) },
            TypeInfo::Jsonb => quote! { floz::Value::OptionJsonb(#expr.clone()) },
            TypeInfo::Ltree => quote! { floz::Value::OptionString(#expr.clone()) },
            TypeInfo::Enum { .. } => quote! { floz::Value::OptionString(#expr.clone().map(|v| v.to_string())) },
            _ => quote! { floz::Value::OptionString(#expr.clone().map(|v| format!("{:?}", v))) },
        };
    }

    match type_info {
        TypeInfo::Integer => quote! { floz::Value::Int(#expr) },
        TypeInfo::Short => quote! { floz::Value::Short(#expr) },
        TypeInfo::BigInt => quote! { floz::Value::BigInt(#expr) },
        TypeInfo::Real => quote! { floz::Value::Real(#expr) },
        TypeInfo::Double => quote! { floz::Value::Double(#expr) },
        TypeInfo::Bool => quote! { floz::Value::Bool(#expr) },
        TypeInfo::Varchar { .. } | TypeInfo::Text | TypeInfo::Ltree => quote! { floz::Value::String(#expr.clone()) },
        TypeInfo::Uuid => quote! { floz::Value::Uuid(#expr) },
        TypeInfo::DateTime => {
            if tz {
                quote! { floz::Value::DateTime(#expr) }
            } else {
                quote! { floz::Value::NaiveDateTime(#expr) }
            }
        }
        TypeInfo::Date => quote! { floz::Value::NaiveDate(#expr) },
        TypeInfo::Time => quote! { floz::Value::NaiveTime(#expr) },
        TypeInfo::Binary => quote! { floz::Value::Bytes(#expr.clone()) },
        TypeInfo::Json => quote! { floz::Value::Json(#expr.clone()) },
        TypeInfo::Jsonb => quote! { floz::Value::Jsonb(#expr.clone()) },
        TypeInfo::Enum { .. } => quote! { floz::Value::String(#expr.to_string()) },
        _ => quote! { floz::Value::String(format!("{:?}", #expr)) },
    }
}

pub fn path_to_table_ident(path: &syn::Path) -> syn::Path {
    let mut p = path.clone();
    if let Some(seg) = p.segments.last_mut() {
        seg.ident = quote::format_ident!("{}Table", seg.ident);
    }
    p
}
