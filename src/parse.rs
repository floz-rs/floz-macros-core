//! Parser for the `schema!` DSL.
//!
//! Parses the token stream from `schema! { ... }` into a `SchemaInput` AST.
//! Uses `syn` for token parsing.

use proc_macro2::Ident;
use syn::parse::{Parse, ParseStream};
use syn::{braced, parenthesized, LitInt, LitStr, Token};

use crate::ast::*;

/// Maximum columns per model (u64 bitmask limit for dirty tracking).
const MAX_COLUMNS: usize = 64;

// ═══════════════════════════════════════════════════════════════
// SchemaInput
// ═══════════════════════════════════════════════════════════════

impl Parse for SchemaInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut models = Vec::new();

        while !input.is_empty() {
            models.push(input.parse::<ModelDef>()?);
        }

        if models.is_empty() {
            return Err(input.error("schema! requires at least one model definition"));
        }

        Ok(SchemaInput { models })
    }
}

// ═══════════════════════════════════════════════════════════════
// ModelDef
// ═══════════════════════════════════════════════════════════════

impl Parse for ModelDef {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Parse optional #[hooks] attribute
        let mut has_custom_hooks = false;
        if input.peek(Token![#]) {
            let attrs = syn::Attribute::parse_outer(input)?;
            for attr in attrs {
                if attr.path().is_ident("hooks") {
                    has_custom_hooks = true;
                } else {
                    return Err(syn::Error::new_spanned(attr, "unknown attribute on model. Did you mean #[hooks]?"));
                }
            }
        }

        // `model`
        let kw: Ident = input.parse()?;
        if kw != "model" {
            return Err(syn::Error::new(kw.span(), "expected `model`"));
        }

        // Model name (e.g., `User`)
        let name: Ident = input.parse()?;

        // Table name in parens: `("users")`
        let table_content;
        parenthesized!(table_content in input);
        let table_lit: LitStr = table_content.parse()?;
        let table_name = table_lit.value();

        // Fields in braces: `{ ... }`
        let body_content;
        braced!(body_content in input);

        let mut db_columns = Vec::new();
        let mut relationships = Vec::new();
        let mut constraints = Vec::new();

        while !body_content.is_empty() {
            // Check for table constraints: @primary_key(...), @unique(...), @index(...)
            if body_content.peek(Token![@]) {
                let constraint = parse_constraint(&body_content)?;
                constraints.push(constraint);
                // Optional trailing comma
                let _ = body_content.parse::<Token![,]>();
                continue;
            }

            // Parse field: `name: type_fn("col", ...args).modifier()...`
            let field = parse_field(&body_content)?;

            match field {
                ParsedField::Column(f) => db_columns.push(f),
                ParsedField::Relationship(r) => relationships.push(r),
            }

            // Optional trailing comma
            let _ = body_content.parse::<Token![,]>();
        }

        // Validate: max 64 columns
        if db_columns.len() > MAX_COLUMNS {
            return Err(syn::Error::new(
                name.span(),
                format!(
                    "Model `{}` has {} columns, but the maximum is {} (u64 bitmask limit). \
                     Consider normalizing your schema.",
                    name, db_columns.len(), MAX_COLUMNS
                ),
            ));
        }

        Ok(ModelDef {
            name,
            table_name,
            db_columns,
            relationships,
            constraints,
            has_custom_hooks,
        })
    }
}

// ═══════════════════════════════════════════════════════════════
// Field Parsing
// ═══════════════════════════════════════════════════════════════

/// A parsed field — either a column or a relationship.
enum ParsedField {
    Column(FieldDef),
    Relationship(RelDef),
}

/// Parse a single field: `name: type_fn("col", ...args).modifier()...`
fn parse_field(input: ParseStream) -> syn::Result<ParsedField> {
    // Rust field name
    let rust_name: Ident = input.parse()?;

    // Colon
    input.parse::<Token![:]>()?;

    // Type function name (e.g., `integer`, `varchar`, `array`)
    let type_fn: Ident = input.parse()?;

    // Type function arguments in parens
    let args_content;
    parenthesized!(args_content in input);

    // Parse based on type function name
    let type_fn_str = type_fn.to_string();

    // Check if this is a relationship
    if type_fn_str == "array" {
        let rel = parse_relationship(rust_name, &args_content)?;
        // Consume any trailing modifiers (shouldn't have any, but be lenient)
        consume_modifiers(input)?;
        return Ok(ParsedField::Relationship(rel));
    }

    // Parse as a column
    let (column_name, type_info) = parse_type_fn(&type_fn_str, &type_fn, &args_content)?;

    // Parse modifier chain: .primary().nullable().auto_increment()...
    let modifiers = parse_modifiers(input)?;

    Ok(ParsedField::Column(FieldDef {
        rust_name,
        column_name,
        type_info,
        modifiers,
    }))
}

/// Parse the type function arguments and return (column_name, TypeInfo).
fn parse_type_fn(
    type_fn_str: &str,
    type_fn_ident: &Ident,
    args: ParseStream,
) -> syn::Result<(String, TypeInfo)> {
    match type_fn_str {
        // Simple types: type("column_name")
        "integer" => {
            let col: LitStr = args.parse()?;
            Ok((col.value(), TypeInfo::Integer))
        }
        "short" => {
            let col: LitStr = args.parse()?;
            Ok((col.value(), TypeInfo::Short))
        }
        "bigint" => {
            let col: LitStr = args.parse()?;
            Ok((col.value(), TypeInfo::BigInt))
        }
        "real" => {
            let col: LitStr = args.parse()?;
            Ok((col.value(), TypeInfo::Real))
        }
        "double" => {
            let col: LitStr = args.parse()?;
            Ok((col.value(), TypeInfo::Double))
        }
        "text" => {
            let col: LitStr = args.parse()?;
            Ok((col.value(), TypeInfo::Text))
        }
        "bool" => {
            let col: LitStr = args.parse()?;
            Ok((col.value(), TypeInfo::Bool))
        }
        "date" => {
            let col: LitStr = args.parse()?;
            Ok((col.value(), TypeInfo::Date))
        }
        "time" => {
            let col: LitStr = args.parse()?;
            Ok((col.value(), TypeInfo::Time))
        }
        "datetime" => {
            let col: LitStr = args.parse()?;
            Ok((col.value(), TypeInfo::DateTime))
        }
        "uuid" => {
            let col: LitStr = args.parse()?;
            Ok((col.value(), TypeInfo::Uuid))
        }
        "binary" => {
            let col: LitStr = args.parse()?;
            Ok((col.value(), TypeInfo::Binary))
        }
        "json" => {
            let col: LitStr = args.parse()?;
            Ok((col.value(), TypeInfo::Json))
        }
        "jsonb" => {
            let col: LitStr = args.parse()?;
            Ok((col.value(), TypeInfo::Jsonb))
        }
        "ltree" => {
            let col: LitStr = args.parse()?;
            Ok((col.value(), TypeInfo::Ltree))
        }

        // enumeration("column", RustType)
        "enumeration" => {
            let col: LitStr = args.parse()?;
            args.parse::<Token![,]>()?;
            let rust_type: syn::Ident = args.parse()?;
            Ok((col.value(), TypeInfo::Enum { rust_type: rust_type.to_string() }))
        }

        // varchar("column", max_length)
        "varchar" => {
            let col: LitStr = args.parse()?;
            args.parse::<Token![,]>()?;
            let len: LitInt = args.parse()?;
            let max_length: u32 = len.base10_parse()?;
            Ok((col.value(), TypeInfo::Varchar { max_length }))
        }

        // decimal("column", precision, scale)
        "decimal" => {
            let col: LitStr = args.parse()?;
            args.parse::<Token![,]>()?;
            let p: LitInt = args.parse()?;
            args.parse::<Token![,]>()?;
            let s: LitInt = args.parse()?;
            Ok((
                col.value(),
                TypeInfo::Decimal {
                    precision: p.base10_parse()?,
                    scale: s.base10_parse()?,
                },
            ))
        }

        // col(RustType, "column") — generic escape hatch
        "col" => {
            let rust_type: Ident = args.parse()?;
            args.parse::<Token![,]>()?;
            let col: LitStr = args.parse()?;
            Ok((col.value(), TypeInfo::Col { rust_type: rust_type.to_string() }))
        }

        // Native PG array types
        "text_array" => {
            let col: LitStr = args.parse()?;
            Ok((col.value(), TypeInfo::TextArray))
        }
        "int_array" => {
            let col: LitStr = args.parse()?;
            Ok((col.value(), TypeInfo::IntArray))
        }
        "bigint_array" => {
            let col: LitStr = args.parse()?;
            Ok((col.value(), TypeInfo::BigIntArray))
        }
        "uuid_array" => {
            let col: LitStr = args.parse()?;
            Ok((col.value(), TypeInfo::UuidArray))
        }
        "bool_array" => {
            let col: LitStr = args.parse()?;
            Ok((col.value(), TypeInfo::BoolArray))
        }
        "real_array" => {
            let col: LitStr = args.parse()?;
            Ok((col.value(), TypeInfo::RealArray))
        }
        "double_array" => {
            let col: LitStr = args.parse()?;
            Ok((col.value(), TypeInfo::DoubleArray))
        }
        "short_array" => {
            let col: LitStr = args.parse()?;
            Ok((col.value(), TypeInfo::ShortArray))
        }
        "varchar_array" => {
            let col: LitStr = args.parse()?;
            Ok((col.value(), TypeInfo::VarcharArray))
        }

        _ => Err(syn::Error::new(
            type_fn_ident.span(),
            format!(
                "unknown type function `{}`\n\
                 available types: integer, short, bigint, real, double, decimal, \
                 varchar, text, bool, date, time, datetime, uuid, binary, col, \
                 text_array, int_array, bigint_array, uuid_array, bool_array, \
                 real_array, double_array, short_array, varchar_array",
                type_fn_str
            ),
        )),
    }
}

/// Parse a relationship: `array(TargetModel, "fk_column")`
fn parse_relationship(rust_name: Ident, args: ParseStream) -> syn::Result<RelDef> {
    let target_model: Ident = args.parse()?;
    args.parse::<Token![,]>()?;
    let fk_col: LitStr = args.parse()?;

    Ok(RelDef {
        rust_name,
        target_model,
        fk_column: fk_col.value(),
    })
}

// ═══════════════════════════════════════════════════════════════
// Modifier Parsing
// ═══════════════════════════════════════════════════════════════

/// Parse a chain of modifiers: `.primary().nullable().auto_increment()`
fn parse_modifiers(input: ParseStream) -> syn::Result<Vec<Modifier>> {
    let mut modifiers = Vec::new();

    while input.peek(Token![.]) {
        input.parse::<Token![.]>()?;
        let modifier_name: Ident = input.parse()?;

        let modifier = match modifier_name.to_string().as_str() {
            "primary" => {
                parse_empty_parens(input)?;
                Modifier::Primary
            }
            "auto_increment" => {
                parse_empty_parens(input)?;
                Modifier::AutoIncrement
            }
            "nullable" => {
                parse_empty_parens(input)?;
                Modifier::Nullable
            }
            "unique" => {
                parse_empty_parens(input)?;
                Modifier::Unique
            }
            "now" => {
                parse_empty_parens(input)?;
                Modifier::Now
            }
            "tz" => {
                parse_empty_parens(input)?;
                Modifier::Tz
            }
            "default" => {
                let content;
                parenthesized!(content in input);
                let val: LitStr = content.parse()?;
                Modifier::Default(val.value())
            }
            other => {
                return Err(syn::Error::new(
                    modifier_name.span(),
                    format!(
                        "unknown modifier `.{}`\n\
                         available modifiers: .primary(), .auto_increment(), .nullable(), \
                         .unique(), .default(\"...\"), .now(), .tz()",
                        other
                    ),
                ));
            }
        };

        modifiers.push(modifier);
    }

    Ok(modifiers)
}

/// Consume modifiers without storing them (for relationships).
fn consume_modifiers(input: ParseStream) -> syn::Result<()> {
    while input.peek(Token![.]) {
        input.parse::<Token![.]>()?;
        let _name: Ident = input.parse()?;
        if input.peek(syn::token::Paren) {
            let _content;
            parenthesized!(_content in input);
        }
    }
    Ok(())
}

/// Parse empty parentheses: `()`
fn parse_empty_parens(input: ParseStream) -> syn::Result<()> {
    let _content;
    parenthesized!(_content in input);
    Ok(())
}

// ═══════════════════════════════════════════════════════════════
// Constraint Parsing
// ═══════════════════════════════════════════════════════════════

/// Parse a table constraint: `@primary_key(col1, col2)`
fn parse_constraint(input: ParseStream) -> syn::Result<TableConstraint> {
    input.parse::<Token![@]>()?;
    let constraint_name: Ident = input.parse()?;

    let content;
    parenthesized!(content in input);

    let mut columns = Vec::new();
    while !content.is_empty() {
        let col: Ident = content.parse()?;
        columns.push(col.to_string());
        if content.peek(Token![,]) {
            content.parse::<Token![,]>()?;
        }
    }

    match constraint_name.to_string().as_str() {
        "primary_key" => Ok(TableConstraint::PrimaryKey(columns)),
        "unique" => Ok(TableConstraint::Unique(columns)),
        "index" => Ok(TableConstraint::Index(columns)),
        other => Err(syn::Error::new(
            constraint_name.span(),
            format!(
                "unknown constraint `@{}`\n\
                 available constraints: @primary_key(), @unique(), @index()",
                other
            ),
        )),
    }
}

// ═══════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════

