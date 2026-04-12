use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use crate::ast::{ModelDef, RelDef};
use super::utils::path_to_table_ident;

pub fn generate_hooks(model: &ModelDef) -> TokenStream {
    if model.has_custom_hooks {
        quote! {} // User provides their own `impl floz::FlozHooks for #name {}`
    } else {
        let name = &model.name;
        quote! {
            impl floz::FlozHooks for #name {}
        }
    }
}

pub fn generate_rel_methods(model: &ModelDef) -> TokenStream {
    let name = &model.name;

    let rel_methods: Vec<TokenStream> = model
        .relationships
        .iter()
        .map(|rel| {
            let fetch_fn = generate_fetch_method(model, rel);
            let preload_fn = generate_preload_method(model, rel);

            quote! {
                #fetch_fn
                #preload_fn
            }
        })
        .collect();

    if rel_methods.is_empty() {
        quote! {}
    } else {
        quote! {
            impl #name {
                #(#rel_methods)*
            }
        }
    }
}

/// Helper to derive the conventional foreign key name for a table (e.g., "users" -> "user_id").
pub fn derive_fk_name(table_name: &str) -> String {
    let mut singular = table_name.to_lowercase();
    if singular.ends_with('s') {
        singular.pop();
    }
    format!("{}_id", singular)
}

/// Helper to derive the conventional foreign key name for a target model struct (e.g., "Role" -> "role_id").
pub fn derive_target_fk_name(path: &syn::Path) -> String {
    if let Some(segment) = path.segments.last() {
        let name = segment.ident.to_string().to_lowercase();
        format!("{}_id", name)
    } else {
        "target_id".to_string()
    }
}

/// Generates the `fetch_{relation}()` lazy-load method.
fn generate_fetch_method(model: &ModelDef, rel: &RelDef) -> TokenStream {
    let rel_name = &rel.rust_name;
    let target = &rel.target_model;

    let pk_struct_field = model
        .primary_key_columns()
        .first()
        .map(|f| &f.rust_name)
        .cloned()
        .unwrap_or_else(|| format_ident!("id"));

    if let crate::ast::RelationType::ManyToMany { through } = &rel.relation_type {
        let fetch_name = format_ident!("fetch_{}", rel_name);
        let through_table = format_ident!("{}Table", through);
        let through_model = format_ident!("{}", through);
        let target_table = path_to_table_ident(target);
        
        let parent_fk = format_ident!("{}", derive_fk_name(&model.table_name));
        let target_fk = format_ident!("{}", derive_target_fk_name(target));

        return quote! {
            /// Lazy-fetch ManyToMany related entities. Executes two optimized ORM queries.
            pub async fn #fetch_name(
                &self,
                db: &impl ::floz::Executor
            ) -> Result<Vec<#target>, ::floz::FlozError> {
                let through_recs = #through_model::filter(#through_table::#parent_fk.eq(self.#pk_struct_field.clone()), db).await?;
                if through_recs.is_empty() { return Ok(vec![]); }
                
                let target_ids: Vec<_> = through_recs.into_iter().map(|rec| rec.#target_fk).collect();
                #target::filter(#target_table::id.in_list(target_ids), db).await
            }
        };
    }

    let target_table = path_to_table_ident(target);
    let fk_col = format_ident!("{}", rel.fk_column);
    let fetch_name = format_ident!("fetch_{}", rel_name);

    quote! {
        /// Lazy-fetch related entities. Executes one query per call.
        pub async fn #fetch_name(
            &self,
            db: &impl ::floz::Executor
        ) -> Result<Vec<#target>, floz::FlozError> {
            #target::filter(#target_table::#fk_col.eq(self.#pk_struct_field.clone()), db).await
        }
    }
}

/// Generates the `preload_{relation}()` batch-load method.
fn generate_preload_method(model: &ModelDef, rel: &RelDef) -> TokenStream {
    let rel_name = &rel.rust_name;
    let target = &rel.target_model;

    // Preload requires a primary key column to extract IDs.
    let Some(pk) = model.primary_key_columns().first().cloned() else {
        return quote! {};
    };

    let pk_struct_field = &pk.rust_name;

    if let crate::ast::RelationType::ManyToMany { through } = &rel.relation_type {
        let preload_name = format_ident!("preload_{}", rel_name);
        let through_table = format_ident!("{}Table", through);
        let through_model = format_ident!("{}", through);
        let target_table = path_to_table_ident(target);
        
        let parent_fk = format_ident!("{}", derive_fk_name(&model.table_name));
        let target_fk = format_ident!("{}", derive_target_fk_name(target));

        return quote! {
            /// Batch-preload ManyToMany related entities.
            pub async fn #preload_name(
                models: &mut [Self],
                db: &impl ::floz::Executor
            ) -> Result<(), ::floz::FlozError> {
                if models.is_empty() { return Ok(()); }
                
                let model_ids: Vec<_> = models.iter().map(|m| m.#pk_struct_field.clone()).collect();
                let through_recs = #through_model::filter(#through_table::#parent_fk.in_list(model_ids), db).await?;
                if through_recs.is_empty() { return Ok(()); }
                
                let mut target_ids: Vec<_> = through_recs.iter().map(|rec| rec.#target_fk.clone()).collect();
                target_ids.sort();
                target_ids.dedup();
                
                let targets = #target::filter(#target_table::id.in_list(target_ids), db).await?;
                let target_map: ::std::collections::HashMap<_, _> = targets.into_iter().map(|t| (t.id.clone(), t)).collect();
                
                let mut relation_map = ::std::collections::HashMap::new();
                for rec in through_recs {
                    if let Some(target) = target_map.get(&rec.#target_fk) {
                        relation_map.entry(rec.#parent_fk.clone())
                            .or_insert_with(::std::vec::Vec::new)
                            .push(target.clone());
                    }
                }
                
                for m in models {
                    if let Some(targets) = relation_map.remove(&m.#pk_struct_field) {
                        m.#rel_name = targets;
                    }
                }
                
                Ok(())
            }
        };
    }

    let target_table = path_to_table_ident(target);
    let fk_col = format_ident!("{}", rel.fk_column);
    let preload_name = format_ident!("preload_{}", rel_name);
    let rel_field = format_ident!("{}", rel_name);
    let pk_rust_name = &pk.rust_name;

    quote! {
        /// Batch-preload related entities to avoid N+1 queries.
        pub async fn #preload_name(
            entities: &mut [Self],
            db: &impl floz::Executor
        ) -> Result<(), floz::FlozError> {
            if entities.is_empty() { 
                return Ok(()); 
            }
            
            // Extract all IDs from the parent slice
            let ids: Vec<_> = entities.iter().map(|e| e.#pk_rust_name.clone()).collect();
            
            // Fetch all related entities in one batch query
            let all_related = #target::filter(#target_table::#fk_col.in_list(ids), db).await?;

            // Group related entities by the foreign key to map them back correctly.
            // (Uses cloned() to distribute into each parent's `Vec`).
            for entity in entities.iter_mut() {
                let entity_id = &entity.#pk_rust_name;
                entity.#rel_field = all_related
                    .iter()
                    .filter(|related| &related.#fk_col == entity_id)
                    .cloned()
                    .collect();
            }

            Ok(())
        }
    }
}
