use crate::ast::{ModelDef, Modifier, TableConstraint, TypeInfo};
use proc_macro2::TokenStream;
use quote::quote;

fn build_create_table_sql(model: &ModelDef) -> String {
    let mut sql = format!("CREATE TABLE IF NOT EXISTS {} (\n", model.table_name);
    let mut columns = Vec::new();

    for field in &model.db_columns {
        let mut col_def = format!("    {}", field.column_name);

        let pg_type = match &field.type_info {
            TypeInfo::Integer if field.is_auto_increment() => "SERIAL".to_string(),
            TypeInfo::BigInt if field.is_auto_increment() => "BIGSERIAL".to_string(),
            TypeInfo::Short if field.is_auto_increment() => "SMALLSERIAL".to_string(),
            TypeInfo::Integer => "INT".to_string(),
            TypeInfo::Short => "SMALLINT".to_string(),
            TypeInfo::BigInt => "BIGINT".to_string(),
            TypeInfo::Real => "REAL".to_string(),
            TypeInfo::Double => "DOUBLE PRECISION".to_string(),
            TypeInfo::Decimal { precision, scale } => format!("NUMERIC({}, {})", precision, scale),
            TypeInfo::Varchar { max_length } => format!("VARCHAR({})", max_length),
            TypeInfo::Text => "TEXT".to_string(),
            TypeInfo::Bool => "BOOLEAN".to_string(),
            TypeInfo::Date => "DATE".to_string(),
            TypeInfo::Time => "TIME".to_string(),
            TypeInfo::DateTime => {
                if field.is_tz() {
                    "TIMESTAMPTZ".to_string()
                } else {
                    "TIMESTAMP".to_string()
                }
            }
            TypeInfo::Uuid => "UUID".to_string(),
            TypeInfo::Binary => "BYTEA".to_string(),
            TypeInfo::Json | TypeInfo::Jsonb => "JSONB".to_string(),
            TypeInfo::Ltree => "LTREE".to_string(),
            TypeInfo::Enum { .. } => "VARCHAR".to_string(),
            TypeInfo::Col { .. } => "TEXT".to_string(),
            TypeInfo::TextArray => "TEXT[]".to_string(),
            TypeInfo::VarcharArray => "VARCHAR[]".to_string(),
            TypeInfo::IntArray => "INT[]".to_string(),
            TypeInfo::ShortArray => "SMALLINT[]".to_string(),
            TypeInfo::BigIntArray => "BIGINT[]".to_string(),
            TypeInfo::UuidArray => "UUID[]".to_string(),
            TypeInfo::BoolArray => "BOOLEAN[]".to_string(),
            TypeInfo::RealArray => "REAL[]".to_string(),
            TypeInfo::DoubleArray => "DOUBLE PRECISION[]".to_string(),
        };

        col_def.push_str(&format!(" {}", pg_type));

        if !field.is_nullable() && !field.is_auto_increment() {
            col_def.push_str(" NOT NULL");
        }

        for modifier in &field.modifiers {
            match modifier {
                Modifier::Primary => col_def.push_str(" PRIMARY KEY"),
                Modifier::Unique => col_def.push_str(" UNIQUE"),
                Modifier::Default(val) => col_def.push_str(&format!(" DEFAULT {}", val)),
                Modifier::Now => col_def.push_str(" DEFAULT CURRENT_TIMESTAMP"),
                _ => {}
            }
        }

        columns.push(col_def);
    }

    for constraint in &model.constraints {
        match constraint {
            TableConstraint::PrimaryKey(cols) => {
                columns.push(format!("    PRIMARY KEY ({})", cols.join(", ")));
            }
            TableConstraint::Unique(cols) => {
                columns.push(format!("    UNIQUE ({})", cols.join(", ")));
            }
            _ => {}
        }
    }

    sql.push_str(&columns.join(",\n"));
    sql.push_str("\n)");

    sql
}

pub fn generate_ddl(model: &ModelDef) -> TokenStream {
    let name = &model.name;
    let create_sql = build_create_table_sql(model);
    let table = &model.table_name;
    let drop_sql = format!("DROP TABLE IF EXISTS {} CASCADE", table);
    let empty_sql = format!("TRUNCATE TABLE {} CASCADE", table);

    quote! {
        impl #name {
            /// Create the table in the database if it does not exist.
            pub async fn create_table(db: &impl floz::Executor) -> Result<(), floz::FlozError> {
                db.execute_raw(#create_sql, vec![]).await?;
                Ok(())
            }

            /// Drop the table from the database if it exists (cascade).
            pub async fn drop_table(db: &impl floz::Executor) -> Result<(), floz::FlozError> {
                db.execute_raw(#drop_sql, vec![]).await?;
                Ok(())
            }

            /// Truncate the table, deleting all rows (cascade).
            pub async fn empty(db: &impl floz::Executor) -> Result<(), floz::FlozError> {
                db.execute_raw(#empty_sql, vec![]).await?;
                Ok(())
            }
        }
    }
}
