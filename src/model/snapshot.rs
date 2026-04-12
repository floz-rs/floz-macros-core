use serde::{Deserialize, Serialize};
use crate::ast::{FieldDef, ModelDef, Modifier, RelDef, TableConstraint, TypeInfo};
use quote::quote;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct SnapshotModel {
    pub name: String,
    pub table_name: String,
    pub db_columns: Vec<SnapshotField>,
    pub relationships: Vec<SnapshotRel>,
    pub constraints: Vec<SnapshotConstraint>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct SnapshotField {
    pub rust_name: String,
    pub column_name: String,
    pub type_info: String,
    pub modifiers: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub renamed_from: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct SnapshotRel {
    pub rust_name: String,
    pub target_model: String,
    pub fk_column: String,
    pub relation_type: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum SnapshotConstraint {
    PrimaryKey(Vec<String>),
    Unique(Vec<String>),
    Index(Vec<String>),
}

// Convert from AST to Snapshot
impl From<&ModelDef> for SnapshotModel {
    fn from(m: &ModelDef) -> Self {
        SnapshotModel {
            name: m.name.to_string(),
            table_name: m.table_name.clone(),
            db_columns: m.db_columns.iter().map(SnapshotField::from).collect(),
            relationships: m.relationships.iter().map(SnapshotRel::from).collect(),
            constraints: m.constraints.iter().map(SnapshotConstraint::from).collect(),
        }
    }
}

impl From<&FieldDef> for SnapshotField {
    fn from(f: &FieldDef) -> Self {
        SnapshotField {
            rust_name: f.rust_name.to_string(),
            column_name: f.column_name.clone(),
            renamed_from: None,
            type_info: match &f.type_info {
                TypeInfo::Integer => "Integer".to_string(),
                TypeInfo::Short => "Short".to_string(),
                TypeInfo::BigInt => "BigInt".to_string(),
                TypeInfo::Real => "Real".to_string(),
                TypeInfo::Double => "Double".to_string(),
                TypeInfo::Decimal { precision, scale } => format!("Decimal({},{})", precision, scale),
                TypeInfo::Varchar { max_length } => format!("Varchar({})", max_length),
                TypeInfo::Text => "Text".to_string(),
                TypeInfo::Bool => "Bool".to_string(),
                TypeInfo::Date => "Date".to_string(),
                TypeInfo::Time => "Time".to_string(),
                TypeInfo::DateTime => "DateTime".to_string(),
                TypeInfo::Uuid => "Uuid".to_string(),
                TypeInfo::Binary => "Binary".to_string(),
                TypeInfo::Json => "Json".to_string(),
                TypeInfo::Jsonb => "Jsonb".to_string(),
                TypeInfo::Ltree => "Ltree".to_string(),
                TypeInfo::Enum { rust_type } => format!("Enum({})", rust_type),
                TypeInfo::Col { rust_type } => format!("Col({})", rust_type),
                _ => "Array".to_string(), // Catch native arrays
            },
            modifiers: f.modifiers.iter().map(|m| match m {
                Modifier::Primary => "Primary".to_string(),
                Modifier::AutoIncrement => "AutoIncrement".to_string(),
                Modifier::Nullable => "Nullable".to_string(),
                Modifier::Unique => "Unique".to_string(),
                Modifier::Default(val) => format!("Default({})", val),
                Modifier::Now => "Now".to_string(),
                Modifier::Tz => "Tz".to_string(),
                Modifier::Index => "Index".to_string(),
                Modifier::References { table, column } => format!("References({},{})", table, column),
                Modifier::OnDelete(action) => format!("OnDelete({})", action),
            }).collect(),
        }
    }
}

impl From<&RelDef> for SnapshotRel {
    fn from(r: &RelDef) -> Self {
        let path = &r.target_model;
        SnapshotRel {
            rust_name: r.rust_name.to_string(),
            target_model: quote!(#path).to_string().replace(" ", ""),
            fk_column: r.fk_column.clone(),
            relation_type: "HasMany".to_string(),
        }
    }
}

impl From<&TableConstraint> for SnapshotConstraint {
    fn from(c: &TableConstraint) -> Self {
        match c {
            TableConstraint::PrimaryKey(cols) => SnapshotConstraint::PrimaryKey(cols.clone()),
            TableConstraint::Unique(cols) => SnapshotConstraint::Unique(cols.clone()),
            TableConstraint::Index(cols) => SnapshotConstraint::Index(cols.clone()),
        }
    }
}
