use floz_macros_core::ast::{FieldDef, ModelDef, Modifier, TypeInfo};
use floz_macros_core::crud::{generate_crud_routes, parse_crud_config, CrudConfig, CrudOp};
use quote::quote;

// ═══════════════════════════════════════════════════════════════
// CrudConfig parsing logic tests
// ═══════════════════════════════════════════════════════════════
// Since parse_crud_config is internal logic used by #[model], we test
// it implicitly by testing the `#[model("notes", crud)]` macro in `test_model.rs`
// or explicitly testing the `CrudConfig` struct methods here.

#[test]
fn test_crud_config_defaults() {
    let config = CrudConfig::default();
    assert_eq!(
        config.operations(),
        vec![
            CrudOp::List,
            CrudOp::Create,
            CrudOp::Get,
            CrudOp::Update,
            CrudOp::Delete
        ]
    );
    assert_eq!(config.base_path("notes"), "/notes");
    assert_eq!(config.tag_name("Note"), "Note");
    assert!(config.auth.is_none());
}

#[test]
fn test_crud_config_custom() {
    let config = CrudConfig {
        tag: Some("CustomNotes".to_string()),
        path: Some("/api/v1/notes".to_string()),
        only: Some(vec![CrudOp::List, CrudOp::Create]),
        exclude: None,
        auth: Some("jwt".to_string()),
    };

    assert_eq!(config.operations(), vec![CrudOp::List, CrudOp::Create]);
    assert_eq!(config.base_path("notes"), "/api/v1/notes");
    assert_eq!(config.tag_name("Note"), "CustomNotes");
}

#[test]
fn test_crud_config_exclude() {
    let config = CrudConfig {
        tag: None,
        path: None,
        only: None,
        exclude: Some(vec![CrudOp::Delete, CrudOp::Update]),
        auth: None,
    };

    assert_eq!(
        config.operations(),
        vec![CrudOp::List, CrudOp::Create, CrudOp::Get]
    );
}

// ═══════════════════════════════════════════════════════════════
// Crud codegen output tests
// ═══════════════════════════════════════════════════════════════

fn dummy_model() -> ModelDef {
    ModelDef {
        name: syn::parse_str("Note").unwrap(),
        table_name: "notes".to_string(),
        db_columns: vec![
            FieldDef {
                rust_name: syn::parse_str("id").unwrap(),
                column_name: "id".to_string(),
                type_info: TypeInfo::Integer,
                modifiers: vec![Modifier::Primary, Modifier::AutoIncrement],
                validations: vec![],
            },
            FieldDef {
                rust_name: syn::parse_str("title").unwrap(),
                column_name: "title".to_string(),
                type_info: TypeInfo::Varchar { max_length: 255 },
                modifiers: vec![],
                validations: vec![],
            },
        ],
        relationships: vec![],
        constraints: vec![],
        has_custom_hooks: false,
        soft_delete: false,
    }
}

#[test]
fn test_crud_codegen_all_routes() {
    let model = dummy_model();
    let config = CrudConfig::default();

    let tokens = generate_crud_routes(&model, &config);
    let out = tokens.to_string();

    assert!(out.contains("fn __crud_note_list"));
    assert!(out.contains("fn __crud_note_create"));
    assert!(out.contains("fn __crud_note_get"));
    assert!(out.contains("fn __crud_note_update"));
    assert!(out.contains("fn __crud_note_delete"));

    // Check inventory registration occurs for all
    assert_eq!(out.matches(":: floz :: inventory :: submit !").count(), 5);
}

#[test]
fn test_crud_codegen_partial() {
    let model = dummy_model();
    let config = CrudConfig {
        tag: None,
        path: None,
        only: Some(vec![CrudOp::List, CrudOp::Get]),
        exclude: None,
        auth: None,
    };

    let tokens = generate_crud_routes(&model, &config);
    let out = tokens.to_string();

    assert!(out.contains("fn __crud_note_list"));
    assert!(out.contains("fn __crud_note_get"));
    assert!(!out.contains("fn __crud_note_create"));
    assert!(!out.contains("fn __crud_note_update"));
    assert!(!out.contains("fn __crud_note_delete"));

    assert_eq!(out.matches(":: floz :: inventory :: submit !").count(), 2);
}

#[test]
fn test_crud_codegen_auth() {
    let model = dummy_model();
    let config = CrudConfig {
        tag: None,
        path: None,
        only: Some(vec![CrudOp::List]),
        exclude: None,
        auth: Some("jwt".to_string()),
    };

    let tokens = generate_crud_routes(&model, &config);
    let out = tokens.to_string();

    // Check that auth expression is passed correctly (Some("jwt"))
    assert!(out.contains(":: core :: option :: Option :: Some (\"jwt\")"));
}
