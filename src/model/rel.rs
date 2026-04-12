use crate::ast::RelDef;

pub(crate) fn parse_rel_attrs(field: &syn::Field, rust_name: syn::Ident) -> syn::Result<Option<RelDef>> {
    for attr in &field.attrs {
        // [1] Check for #[rel(...)]
        if attr.path().is_ident("rel") {
            let mut relation_type = None;
            let mut target_model: Option<syn::Path> = None;
            let mut fk_column: Option<String> = None;

            attr.parse_nested_meta(|meta| {
                let ident = meta.path.get_ident()
                    .ok_or_else(|| meta.error("expected rel identifier"))?
                    .to_string();

                if ident == "has_many" {
                    relation_type = Some(crate::ast::RelationType::HasMany);
                } else if ident == "belongs_to" {
                    relation_type = Some(crate::ast::RelationType::BelongsTo);
                } else {
                    return Err(meta.error("expected `has_many` or `belongs_to` in #[rel(...)]"));
                }

                // Parse inner parens: (model = "...", foreign_key = "...")
                let content;
                syn::parenthesized!(content in meta.input);

                while !content.is_empty() {
                    let key: syn::Ident = content.parse()?;
                    content.parse::<syn::Token![=]>()?;
                    let val: syn::LitStr = content.parse()?;

                    if key == "model" {
                        target_model = Some(val.parse()?);
                    } else if key == "foreign_key" {
                        fk_column = Some(val.value());
                    } else {
                        return Err(syn::Error::new_spanned(key, "expected `model` or `foreign_key`"));
                    }

                    if content.peek(syn::Token![,]) {
                        content.parse::<syn::Token![,]>()?;
                    }
                }

                Ok(())
            })?;

            if let (Some(rel_type), Some(target), Some(fk)) = (relation_type, target_model, fk_column) {
                return Ok(Some(RelDef {
                    rust_name,
                    target_model: target,
                    fk_column: fk,
                    relation_type: rel_type,
                }));
            } else {
                return Err(syn::Error::new_spanned(attr, "missing `model` or `foreign_key` inside #[rel(...)]"));
            }
        }
        
        // [2] Check for #[m2m(Role, through = "user_roles")]
        if attr.path().is_ident("m2m") {
            let mut target_model: Option<syn::Path> = None;
            let mut through_table: Option<String> = None;

            attr.parse_nested_meta(|meta| {
                if target_model.is_none() {
                    // First arg is the path to the model, e.g. `Role` or `crate::app::Role`
                    target_model = Some(meta.path.clone());
                    return Ok(());
                }

                // Any subsequent args MUST be `through = "table"`
                if meta.path.is_ident("through") {
                    let value = meta.value()?;
                    let lit: syn::LitStr = value.parse()?;
                    through_table = Some(lit.value());
                } else {
                    return Err(meta.error("expected `through = \"...\"` inside #[m2m(...)]"));
                }

                Ok(())
            })?;

            if let Some(target) = target_model {
                return Ok(Some(RelDef {
                    rust_name,
                    target_model: target,
                    fk_column: "".to_string(), // handled specially
                    relation_type: crate::ast::RelationType::ManyToMany {
                        through: through_table.unwrap_or_else(|| "auto_junction".to_string()),
                    },
                }));
            } else {
                return Err(syn::Error::new_spanned(attr, "missing target model in #[m2m(...)]"));
            }
        }
    }

    Ok(None)
}
