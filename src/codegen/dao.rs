use super::utils::{pk_query_parts, quote_table_str, to_value_expr};
use crate::ast::{FieldDef, ModelDef};
use proc_macro2::TokenStream;
use quote::quote;

pub fn generate_dao(model: &ModelDef) -> TokenStream {
    let name = &model.name;

    // Methods available for all models
    let create_fn = generate_create(model);
    let all_fn = generate_all(model);
    let filter_fn = generate_filter(model);
    let paginate_fn = generate_paginate(model);

    // Methods only available for models with a primary key
    let pk_methods = if model.has_primary_key() {
        let pk_cols = model.primary_key_columns();
        let get_fn = generate_get(model, &pk_cols);
        let find_fn = generate_find(model, &pk_cols);
        let save_fn = generate_save(model, &pk_cols);
        let delete_fn = generate_delete(model, &pk_cols);
        quote! {
            #get_fn
            #find_fn
            #save_fn
            #delete_fn
        }
    } else {
        quote! {}
    };

    quote! {
        impl #name {
            #create_fn
            #all_fn
            #filter_fn
            #paginate_fn
            #pk_methods
        }
    }
}

fn generate_paginate(model: &ModelDef) -> TokenStream {
    let table = &model.table_name;
    let soft_delete_filter = if model.soft_delete {
        quote! { .filter_expr(floz::expr::col("deleted_at").is_null()) }
    } else {
        quote! {}
    };

    quote! {
        /// Create a paginated query builder for this model.
        pub fn paginate() -> floz::PaginateQuery<Self> {
            floz::PaginateQuery::new(#table)#soft_delete_filter
        }
    }
}

/// Generate `create()` — INSERT non-auto-increment fields, RETURNING *.
fn generate_create(model: &ModelDef) -> TokenStream {
    let table = quote_table_str(&model.table_name);

    // Columns to insert: all except auto_increment
    let insert_fields: Vec<&FieldDef> = model
        .db_columns
        .iter()
        .filter(|f| !f.is_auto_increment())
        .collect();

    // Build SQL: INSERT INTO table (col1, col2) VALUES ($1, $2) RETURNING *
    let col_list = insert_fields
        .iter()
        .map(|f| f.column_name.as_str())
        .collect::<Vec<_>>()
        .join(", ");

    let param_list = (1..=insert_fields.len())
        .map(|i| format!("${}", i))
        .collect::<Vec<_>>()
        .join(", ");

    let sql = format!(
        "INSERT INTO {} ({}) VALUES ({}) RETURNING *",
        table, col_list, param_list
    );

    // Build params vec
    let param_exprs: Vec<TokenStream> = insert_fields
        .iter()
        .map(|f| to_value_expr(f, quote! { self }))
        .collect();

    quote! {
        /// Insert this entity into the database, returning the full row
        /// (with auto-generated columns like `id`).
        pub async fn create(&self, db: &impl floz::Executor) -> Result<Self, floz::FlozError> {
            floz::FlozHooks::before_create(self)?;
            let params: Vec<floz::Value> = vec![#(#param_exprs),*];
            let result = db.fetch_one(#sql, params).await?;
            floz::FlozHooks::after_create(&result);
            Ok(result)
        }
    }
}

fn generate_all(model: &ModelDef) -> TokenStream {
    let table = quote_table_str(&model.table_name);
    let sql = if model.soft_delete {
        format!("SELECT * FROM {} WHERE deleted_at IS NULL", table)
    } else {
        format!("SELECT * FROM {}", table)
    };

    quote! {
        /// Fetch all rows from the table.
        pub async fn all(db: &impl floz::Executor) -> Result<Vec<Self>, floz::FlozError> {
            db.fetch_all(#sql, vec![]).await
        }
    }
}

fn generate_filter(model: &ModelDef) -> TokenStream {
    let table = quote_table_str(&model.table_name);
    let prefix = if model.soft_delete {
        format!("SELECT * FROM {} WHERE deleted_at IS NULL AND ", table)
    } else {
        format!("SELECT * FROM {} WHERE ", table)
    };

    quote! {
        /// Fetch all rows matching the given filter expression.
        pub async fn filter(
            expr: floz::Expr,
            db: &impl floz::Executor,
        ) -> Result<Vec<Self>, floz::FlozError> {
            let mut sql = String::from(#prefix);
            let mut params = Vec::new();
            let mut idx = 0usize;
            expr.to_sql(&mut sql, &mut params, &mut idx);
            db.fetch_all(&sql, params).await
        }
    }
}

/// Generate `get()` — SELECT * FROM table WHERE pk = $1. Returns error if not found.
fn generate_get(model: &ModelDef, pk_cols: &[&FieldDef]) -> TokenStream {
    let table = quote_table_str(&model.table_name);

    let (fn_params, mut where_clause, param_exprs) = pk_query_parts(pk_cols);
    if model.soft_delete {
        where_clause = format!("{} AND deleted_at IS NULL", where_clause);
    }
    let sql = format!("SELECT * FROM {} WHERE {}", table, where_clause);

    quote! {
        /// Fetch a single row by primary key. Returns `FlozError::NotFound` if missing.
        pub async fn get(#(#fn_params,)* db: &impl floz::Executor) -> Result<Self, floz::FlozError> {
            let params: Vec<floz::Value> = vec![#(#param_exprs),*];
            db.fetch_one(#sql, params).await
        }
    }
}

/// Generate `find()` — like get() but returns Option<Self>.
fn generate_find(model: &ModelDef, pk_cols: &[&FieldDef]) -> TokenStream {
    let table = quote_table_str(&model.table_name);

    let (fn_params, mut where_clause, param_exprs) = pk_query_parts(pk_cols);
    if model.soft_delete {
        where_clause = format!("{} AND deleted_at IS NULL", where_clause);
    }
    let sql = format!("SELECT * FROM {} WHERE {}", table, where_clause);

    quote! {
        /// Fetch a single row by primary key, returning `None` if not found.
        pub async fn find(#(#fn_params,)* db: &impl floz::Executor) -> Result<Option<Self>, floz::FlozError> {
            let params: Vec<floz::Value> = vec![#(#param_exprs),*];
            db.fetch_optional(#sql, params).await
        }
    }
}

/// Generate `save()` — UPDATE only dirty fields WHERE pk.
fn generate_save(model: &ModelDef, pk_cols: &[&FieldDef]) -> TokenStream {
    let table = quote_table_str(&model.table_name);

    // Generate a conditional SET block for each column
    let set_blocks: Vec<TokenStream> = model
        .db_columns
        .iter()
        .enumerate()
        .map(|(idx, field)| {
            let bit: u64 = 1u64 << idx;
            let col_name = &field.column_name;
            let value_expr = to_value_expr(field, quote! { self });

            quote! {
                if self._dirty_flags & #bit != 0 {
                    if !first { sql.push_str(", "); }
                    param_idx += 1;
                    sql.push_str(&format!("{} = ${}", #col_name, param_idx));
                    params.push(#value_expr);
                    first = false;
                }
            }
        })
        .collect();

    // WHERE clause for PK
    let pk_where_parts: Vec<TokenStream> = pk_cols
        .iter()
        .enumerate()
        .map(|(i, pk)| {
            let col_name = &pk.column_name;
            let value_expr = to_value_expr(pk, quote! { self });
            if i == 0 {
                quote! {
                    param_idx += 1;
                    sql.push_str(&format!(" WHERE {} = ${}", #col_name, param_idx));
                    params.push(#value_expr);
                }
            } else {
                quote! {
                    param_idx += 1;
                    sql.push_str(&format!(" AND {} = ${}", #col_name, param_idx));
                    params.push(#value_expr);
                }
            }
        })
        .collect();

    let table_str = format!("UPDATE {} SET ", table);

    quote! {
        /// Update only the dirty (modified) fields in the database.
        /// Does nothing if no fields have been modified.
        pub async fn save(&mut self, db: &impl floz::Executor) -> Result<(), floz::FlozError> {
            floz::FlozHooks::before_save(self)?;

            if !self.is_dirty() {
                floz::FlozHooks::after_save(self);
                return Ok(());
            }

            let mut sql = String::from(#table_str);
            let mut params: Vec<floz::Value> = Vec::new();
            let mut param_idx = 0usize;
            let mut first = true;

            #(#set_blocks)*

            // If nothing was actually dirty (shouldn't happen due to check above)
            if first {
                floz::FlozHooks::after_save(self);
                return Ok(());
            }

            // Add WHERE pk = $N
            #(#pk_where_parts)*

            db.execute_raw(&sql, params).await?;
            self.clear_dirty();

            floz::FlozHooks::after_save(self);
            Ok(())
        }
    }
}

/// Generate `delete()` — DELETE FROM table WHERE pk.
fn generate_delete(model: &ModelDef, pk_cols: &[&FieldDef]) -> TokenStream {
    let table = quote_table_str(&model.table_name);

    let (_, mut where_clause, _) = pk_query_parts(pk_cols);
    let sql = if model.soft_delete {
        format!(
            "UPDATE {} SET deleted_at = NOW() WHERE {}",
            table, where_clause
        )
    } else {
        format!("DELETE FROM {} WHERE {}", table, where_clause)
    };

    let param_exprs: Vec<TokenStream> = pk_cols
        .iter()
        .map(|f| to_value_expr(f, quote! { self }))
        .collect();

    quote! {
        /// Delete this entity from the database by primary key.
        pub async fn delete(&self, db: &impl floz::Executor) -> Result<(), floz::FlozError> {
            floz::FlozHooks::before_delete(self)?;
            let params: Vec<floz::Value> = vec![#(#param_exprs),*];
            db.execute_raw(#sql, params).await?;
            floz::FlozHooks::after_delete(self);
            Ok(())
        }
    }
}
