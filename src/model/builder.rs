use super::col::{apply_type_overrides, extract_col_name, parse_col_attrs};
use super::rel::parse_rel_attrs;
use super::types::resolve_type;
use crate::ast::{FieldDef, ModelDef, Modifier};
use syn::{Fields, ItemStruct};

use quote::format_ident;

/// Build a `ModelDef` from a parsed struct + its `#[col(...)]` field attributes.
pub(crate) fn build_model_def(
    input: &ItemStruct,
    table_name: &str,
    soft_delete: bool,
) -> syn::Result<ModelDef> {
    let fields = match &input.fields {
        Fields::Named(named) => &named.named,
        _ => {
            return Err(syn::Error::new_spanned(
                input,
                "#[model] only supports structs with named fields",
            ))
        }
    };

    let mut db_columns = Vec::new();
    let mut relationships = Vec::new();

    for field in fields {
        let rust_name = field
            .ident
            .clone()
            .ok_or_else(|| syn::Error::new_spanned(field, "expected named field"))?;

        // First check if this field is an explicit relationship marker
        if let Some(rel_def) = parse_rel_attrs(field, rust_name.clone())? {
            relationships.push(rel_def);
            continue;
        }

        // Parse the type to determine TypeInfo
        let (type_info, is_nullable, is_tz) = resolve_type(&field.ty)?;

        // Parse #[col(...)] attributes
        let col_result = parse_col_attrs(field)?;
        let mut modifiers = col_result.modifiers;
        let validations = col_result.validations;

        // If type is Option<T>, add Nullable modifier
        if is_nullable {
            modifiers.push(Modifier::Nullable);
        }

        // If type is TimestampTz, add Tz modifier
        if is_tz {
            modifiers.push(Modifier::Tz);
        }

        // Apply max_length override from #[col(max = N)]
        let type_info = apply_type_overrides(type_info, &modifiers);

        // Determine column name: #[col(name = "...")] or rust field name
        let column_name = extract_col_name(&modifiers).unwrap_or_else(|| rust_name.to_string());

        db_columns.push(FieldDef {
            rust_name,
            column_name,
            type_info,
            modifiers,
            validations,
        });
    }

    if soft_delete && !db_columns.iter().any(|c| c.rust_name == "deleted_at") {
        db_columns.push(FieldDef {
            rust_name: format_ident!("deleted_at"),
            column_name: "deleted_at".to_string(),
            type_info: crate::ast::TypeInfo::DateTime,
            modifiers: vec![Modifier::Nullable, Modifier::Tz],
            validations: vec![],
        });
    }

    // Validate column count
    if db_columns.len() > 64 {
        return Err(syn::Error::new_spanned(
            input,
            format!(
                "Model `{}` has {} columns, but the maximum is 64 (u64 bitmask limit). \
                 Consider normalizing your schema.",
                input.ident,
                db_columns.len()
            ),
        ));
    }

    // Check if user has a custom FlozHooks impl via #[hooks] on the struct
    let has_custom_hooks = input.attrs.iter().any(|a| a.path().is_ident("hooks"));

    Ok(ModelDef {
        name: input.ident.clone(),
        table_name: table_name.to_string(),
        db_columns,
        relationships,
        constraints: Vec::new(), // table constraints from struct attrs (future)
        has_custom_hooks,
        soft_delete,
    })
}
