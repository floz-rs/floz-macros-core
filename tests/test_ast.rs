use floz_macros_core::ast::{FieldDef, ModelDef, Modifier, TableConstraint, TypeInfo};
use proc_macro2::{Ident, Span};

fn make_ident(name: &str) -> Ident {
    Ident::new(name, Span::call_site())
}

#[test]
fn test_field_def_modifiers() {
    let mut field = FieldDef {
        rust_name: make_ident("id"),
        column_name: "id".to_string(),
        type_info: TypeInfo::Integer,
        modifiers: vec![],
    };

    assert!(!field.is_primary());
    assert!(!field.is_auto_increment());
    assert!(!field.is_nullable());
    assert!(!field.is_unique());
    assert!(!field.is_tz());

    field.modifiers.push(Modifier::Primary);
    field.modifiers.push(Modifier::AutoIncrement);
    field.modifiers.push(Modifier::Nullable);
    field.modifiers.push(Modifier::Unique);
    field.modifiers.push(Modifier::Tz);

    assert!(field.is_primary());
    assert!(field.is_auto_increment());
    assert!(field.is_nullable());
    assert!(field.is_unique());
    assert!(field.is_tz());
}

#[test]
fn test_model_def_primary_key() {
    let field_id = FieldDef {
        rust_name: make_ident("id"),
        column_name: "id".to_string(),
        type_info: TypeInfo::Integer,
        modifiers: vec![Modifier::Primary],
    };

    let field_name = FieldDef {
        rust_name: make_ident("name"),
        column_name: "name".to_string(),
        type_info: TypeInfo::Text,
        modifiers: vec![],
    };

    let model = ModelDef {
        name: make_ident("User"),
        table_name: "users".to_string(),
        db_columns: vec![field_id, field_name],
        relationships: vec![],
        constraints: vec![],
        has_custom_hooks: false,
    };

    assert!(model.has_primary_key());
    let pk_cols = model.primary_key_columns();
    assert_eq!(pk_cols.len(), 1);
    assert_eq!(pk_cols[0].rust_name.to_string(), "id");
}

#[test]
fn test_model_def_composite_primary_key() {
    let field_c1 = FieldDef {
        rust_name: make_ident("c1"),
        column_name: "c1".to_string(),
        type_info: TypeInfo::Integer,
        modifiers: vec![],
    };

    let field_c2 = FieldDef {
        rust_name: make_ident("c2"),
        column_name: "c2".to_string(),
        type_info: TypeInfo::Integer,
        modifiers: vec![],
    };

    let model = ModelDef {
        name: make_ident("Composite"),
        table_name: "composites".to_string(),
        db_columns: vec![field_c1, field_c2],
        relationships: vec![],
        constraints: vec![TableConstraint::PrimaryKey(vec![
            "c1".to_string(),
            "c2".to_string(),
        ])],
        has_custom_hooks: false,
    };

    assert!(model.has_primary_key());
    let pk_cols = model.primary_key_columns();
    assert_eq!(pk_cols.len(), 2);
    assert_eq!(pk_cols[0].rust_name.to_string(), "c1");
    assert_eq!(pk_cols[1].rust_name.to_string(), "c2");
}
