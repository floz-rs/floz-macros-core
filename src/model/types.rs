use quote::quote;
use syn::Type;
use crate::ast::TypeInfo;

/// Resolve a Rust type token into a TypeInfo + nullable flag + tz flag.
///
/// The proc macro works on **tokens**, not resolved types, so `Varchar` and
/// `String` are different tokens even though `type Varchar = String`.
pub(crate) fn resolve_type(ty: &Type) -> syn::Result<(TypeInfo, bool, bool)> {
    // Check for Option<T>
    if let Some(inner) = extract_option_inner(ty) {
        let (type_info, _, is_tz) = resolve_type(inner)?;
        return Ok((type_info, true, is_tz));
    }

    let type_name = type_to_string(ty);

    match type_name.as_str() {
        // String types
        "Varchar" => Ok((TypeInfo::Varchar { max_length: 255 }, false, false)),
        "String" => Ok((TypeInfo::Varchar { max_length: 255 }, false, false)),
        "Text" => Ok((TypeInfo::Text, false, false)),

        // Integer types
        "i32" => Ok((TypeInfo::Integer, false, false)),
        "i16" => Ok((TypeInfo::Short, false, false)),
        "i64" => Ok((TypeInfo::BigInt, false, false)),

        // Float types
        "f32" => Ok((TypeInfo::Real, false, false)),
        "f64" => Ok((TypeInfo::Double, false, false)),
        "Decimal" => Ok((TypeInfo::Decimal { precision: 10, scale: 2 }, false, false)),

        // Boolean
        "bool" => Ok((TypeInfo::Bool, false, false)),

        // Temporal
        "Timestamp" | "NaiveDateTime" | "chrono :: NaiveDateTime" => {
            Ok((TypeInfo::DateTime, false, false))
        }
        "TimestampTz" | "DateTime < Utc >" | "chrono :: DateTime < chrono :: Utc >" => {
            Ok((TypeInfo::DateTime, false, true))
        }
        "Date" | "NaiveDate" | "chrono :: NaiveDate" => {
            Ok((TypeInfo::Date, false, false))
        }
        "NaiveTime" | "chrono :: NaiveTime" => {
            Ok((TypeInfo::Time, false, false))
        }

        // UUID
        "Uuid" | "uuid :: Uuid" => Ok((TypeInfo::Uuid, false, false)),

        // Binary
        "Bytes" | "Vec < u8 >" => Ok((TypeInfo::Binary, false, false)),

        // JSON
        "Json" => Ok((TypeInfo::Json, false, false)),
        "Jsonb" => Ok((TypeInfo::Jsonb, false, false)),
        "serde_json :: Value" | "Value" => Ok((TypeInfo::Jsonb, false, false)),

        // Array types
        "Vec < String >" => Ok((TypeInfo::TextArray, false, false)),
        "Vec < Uuid >" | "Vec < uuid :: Uuid >" => Ok((TypeInfo::UuidArray, false, false)),
        "Vec < i32 >" => Ok((TypeInfo::IntArray, false, false)),
        "Vec < i64 >" => Ok((TypeInfo::BigIntArray, false, false)),
        "Vec < i16 >" => Ok((TypeInfo::ShortArray, false, false)),
        "Vec < bool >" => Ok((TypeInfo::BoolArray, false, false)),
        "Vec < f32 >" => Ok((TypeInfo::RealArray, false, false)),
        "Vec < f64 >" => Ok((TypeInfo::DoubleArray, false, false)),

        _ => {
            Err(syn::Error::new_spanned(
                ty,
                format!(
                    "unknown type `{}` in #[model] struct.\n\
                     Supported types: Varchar, Text, String, i32, i64, i16, f32, f64, Decimal, \
                     bool, Timestamp, TimestampTz, Date, Uuid, Json, Jsonb, Bytes, \
                     Vec<String>, Vec<i32>, Vec<i64>, and Option<T> for any of these.",
                    type_name
                ),
            ))
        }
    }
}

/// Extract the inner type from `Option<T>`.
pub(crate) fn extract_option_inner(ty: &Type) -> Option<&Type> {
    if let Type::Path(type_path) = ty {
        let seg = type_path.path.segments.last()?;
        if seg.ident == "Option" {
            if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                if let Some(syn::GenericArgument::Type(inner)) = args.args.first() {
                    return Some(inner);
                }
            }
        }
    }
    None
}

/// Convert a syn::Type to a normalized string for matching.
fn type_to_string(ty: &Type) -> String {
    quote!(#ty)
        .to_string()
        .replace(" < ", " < ")
        .replace(" > ", " > ")
}
