//! Validation code generation.
//!
//! Generates a `validate()` method on the model struct based on
//! `#[col(...)]` validation rules (len, email, url, range, regex, required).
//!
//! The generated code is pure Rust — no external validation crate.
//! Uses `floz::ValidationErrors` for error accumulation.

use crate::ast::{ModelDef, ValidationRule};
use proc_macro2::TokenStream;
use quote::quote;

/// Generate a `validate()` method if any field has validation rules.
pub fn generate_validate(model: &ModelDef) -> TokenStream {
    // Collect all fields that have at least one validation rule
    let has_validations = model.db_columns.iter().any(|f| !f.validations.is_empty());

    if !has_validations {
        return quote! {};
    }

    let name = &model.name;

    // Generate validation checks for each field
    let field_checks: Vec<TokenStream> = model
        .db_columns
        .iter()
        .filter(|f| !f.validations.is_empty())
        .flat_map(|field| {
            let field_name = &field.rust_name;
            let field_name_str = field_name.to_string();
            let is_nullable = field
                .modifiers
                .iter()
                .any(|m| matches!(m, crate::ast::Modifier::Nullable));

            field.validations.iter().map(move |rule| {
                match rule {
                    ValidationRule::Length { min, max } => {
                        let mut checks = Vec::new();
                        if is_nullable {
                            // For Option<String> fields
                            if let Some(min_val) = min {
                                let min_usize = *min_val as usize;
                                let msg = format!("length must be at least {}", min_val);
                                checks.push(quote! {
                                    if let Some(ref __v) = self.#field_name {
                                        if __v.len() < #min_usize {
                                            __errors.add(#field_name_str, #msg);
                                        }
                                    }
                                });
                            }
                            if let Some(max_val) = max {
                                let max_usize = *max_val as usize;
                                let msg = format!("length must be at most {}", max_val);
                                checks.push(quote! {
                                    if let Some(ref __v) = self.#field_name {
                                        if __v.len() > #max_usize {
                                            __errors.add(#field_name_str, #msg);
                                        }
                                    }
                                });
                            }
                        } else {
                            if let Some(min_val) = min {
                                let min_usize = *min_val as usize;
                                let msg = format!("length must be at least {}", min_val);
                                checks.push(quote! {
                                    if self.#field_name.len() < #min_usize {
                                        __errors.add(#field_name_str, #msg);
                                    }
                                });
                            }
                            if let Some(max_val) = max {
                                let max_usize = *max_val as usize;
                                let msg = format!("length must be at most {}", max_val);
                                checks.push(quote! {
                                    if self.#field_name.len() > #max_usize {
                                        __errors.add(#field_name_str, #msg);
                                    }
                                });
                            }
                        }
                        quote! { #(#checks)* }
                    }

                    ValidationRule::Email => {
                        let msg = "must be a valid email address";
                        if is_nullable {
                            quote! {
                                if let Some(ref __v) = self.#field_name {
                                    if !floz::validators::is_email(__v) {
                                        __errors.add(#field_name_str, #msg);
                                    }
                                }
                            }
                        } else {
                            quote! {
                                if !floz::validators::is_email(&self.#field_name) {
                                    __errors.add(#field_name_str, #msg);
                                }
                            }
                        }
                    }

                    ValidationRule::Url => {
                        let msg = "must be a valid URL";
                        if is_nullable {
                            quote! {
                                if let Some(ref __v) = self.#field_name {
                                    if !floz::validators::is_url(__v) {
                                        __errors.add(#field_name_str, #msg);
                                    }
                                }
                            }
                        } else {
                            quote! {
                                if !floz::validators::is_url(&self.#field_name) {
                                    __errors.add(#field_name_str, #msg);
                                }
                            }
                        }
                    }

                    ValidationRule::Range { min, max } => {
                        let mut checks = Vec::new();
                        if let Some(min_val) = min {
                            let msg = format!("must be at least {}", min_val);
                            if is_nullable {
                                checks.push(quote! {
                                    if let Some(ref __v) = self.#field_name {
                                        if (*__v as f64) < #min_val {
                                            __errors.add(#field_name_str, #msg);
                                        }
                                    }
                                });
                            } else {
                                checks.push(quote! {
                                    if (self.#field_name as f64) < #min_val {
                                        __errors.add(#field_name_str, #msg);
                                    }
                                });
                            }
                        }
                        if let Some(max_val) = max {
                            let msg = format!("must be at most {}", max_val);
                            if is_nullable {
                                checks.push(quote! {
                                    if let Some(ref __v) = self.#field_name {
                                        if (*__v as f64) > #max_val {
                                            __errors.add(#field_name_str, #msg);
                                        }
                                    }
                                });
                            } else {
                                checks.push(quote! {
                                    if (self.#field_name as f64) > #max_val {
                                        __errors.add(#field_name_str, #msg);
                                    }
                                });
                            }
                        }
                        quote! { #(#checks)* }
                    }

                    ValidationRule::Regex(pattern) => {
                        let msg = format!("must match pattern: {}", pattern);
                        if is_nullable {
                            quote! {
                                if let Some(ref __v) = self.#field_name {
                                    if !floz::validators::matches_regex(__v, #pattern) {
                                        __errors.add(#field_name_str, #msg);
                                    }
                                }
                            }
                        } else {
                            quote! {
                                if !floz::validators::matches_regex(&self.#field_name, #pattern) {
                                    __errors.add(#field_name_str, #msg);
                                }
                            }
                        }
                    }

                    ValidationRule::Required => {
                        let msg = "is required";
                        // Only meaningful on Option<T> fields
                        if is_nullable {
                            quote! {
                                if self.#field_name.is_none() {
                                    __errors.add(#field_name_str, #msg);
                                }
                            }
                        } else {
                            // Non-optional fields are always present — no-op
                            quote! {}
                        }
                    }
                }
            }).collect::<Vec<_>>()
        })
        .collect();

    quote! {
        impl #name {
            /// Validate this model's fields against the rules defined in `#[col(...)]`.
            ///
            /// Returns `Ok(())` if all validations pass, or `Err(ValidationErrors)`
            /// with per-field error messages. When used with `?` in a handler, this
            /// automatically returns a 422 response via `ApiError` integration.
            ///
            /// ```ignore
            /// #[route(post: "/users")]
            /// async fn create_user(body: Json<User>, state: State) -> Resp {
            ///     body.validate()?;  // auto 422 on failure
            ///     // ...
            /// }
            /// ```
            pub fn validate(&self) -> ::core::result::Result<(), floz::ValidationErrors> {
                let mut __errors = floz::ValidationErrors::new();
                #(#field_checks)*
                __errors.into_result()
            }
        }
    }
}
