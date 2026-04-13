use crate::ast::ModelDef;
use crate::model::attr::parse_model_attr;
use crate::model::builder::build_model_def;
use syn::{parse_file, Item};

/// Extracts all `#[model]` definitions from a given Rust source code string.
pub fn extract_models_from_source(source: &str) -> Result<Vec<ModelDef>, String> {
    let mut models = Vec::new();
    let file = parse_file(source).map_err(|e| format!("Failed to parse source: {}", e))?;

    for item in file.items {
        if let Item::Struct(item_struct) = &item {
            for attr in &item_struct.attrs {
                if attr.path().is_ident("model") {
                    if let syn::Meta::List(list) = &attr.meta {
                        let (table_name, _, soft_delete) = parse_model_attr(list.tokens.clone())
                            .map_err(|e| format!("Failed to parse #[model] attr: {}", e))?;

                        let model_def = build_model_def(item_struct, &table_name, soft_delete)
                            .map_err(|e| format!("Failed to build model def: {}", e))?;

                        models.push(model_def);
                    }
                }
            }
        }
    }

    Ok(models)
}
