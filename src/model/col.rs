use syn::{LitInt, LitStr, LitFloat};
use crate::ast::{Modifier, TypeInfo, ValidationRule};

/// Internal-only modifier for `#[col(name = "...")]` (not a DB modifier).
#[allow(dead_code)]
#[derive(Debug)]
enum ColOverride {
    Name(String),
    Max(u32),
    Precision(u32),
    Scale(u32),
}

/// Result of parsing `#[col(...)]` — DB modifiers + validation rules.
pub(crate) struct ColParseResult {
    pub modifiers: Vec<Modifier>,
    pub validations: Vec<ValidationRule>,
}

/// Parse all `#[col(...)]` attributes on a field into modifiers + validations.
pub(crate) fn parse_col_attrs(field: &syn::Field) -> syn::Result<ColParseResult> {
    let mut modifiers = Vec::new();
    let mut validations = Vec::new();

    for attr in &field.attrs {
        if !attr.path().is_ident("col") {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            let ident = meta.path.get_ident()
                .ok_or_else(|| meta.error("expected identifier"))?;

            match ident.to_string().as_str() {
                // ── DB modifiers ──
                "key" => modifiers.push(Modifier::Primary),
                "auto" => modifiers.push(Modifier::AutoIncrement),
                "unique" => modifiers.push(Modifier::Unique),
                "index" => modifiers.push(Modifier::Index),
                "now" => modifiers.push(Modifier::Now),

                "default" => {
                    let value = meta.value()?;
                    let lit: LitStr = value.parse()?;
                    modifiers.push(Modifier::Default(lit.value()));
                }

                "name" => {
                    let value = meta.value()?;
                    let lit: LitStr = value.parse()?;
                    // Store as a special Default placeholder — extracted later
                    modifiers.push(Modifier::Default(format!("__col_name__{}", lit.value())));
                }

                "max" => {
                    let value = meta.value()?;
                    let lit: LitInt = value.parse()?;
                    let n: u32 = lit.base10_parse()?;
                    // Store as a special tag — applied later in apply_type_overrides
                    modifiers.push(Modifier::Default(format!("__col_max__{}", n)));
                }

                "precision" => {
                    let value = meta.value()?;
                    let lit: LitInt = value.parse()?;
                    let n: u32 = lit.base10_parse()?;
                    modifiers.push(Modifier::Default(format!("__col_precision__{}", n)));
                }

                "scale" => {
                    let value = meta.value()?;
                    let lit: LitInt = value.parse()?;
                    let n: u32 = lit.base10_parse()?;
                    modifiers.push(Modifier::Default(format!("__col_scale__{}", n)));
                }

                "references" => {
                    // references("table", "col")
                    let content;
                    syn::parenthesized!(content in meta.input);
                    let table: LitStr = content.parse()?;
                    content.parse::<syn::Token![,]>()?;
                    let column: LitStr = content.parse()?;
                    modifiers.push(Modifier::References {
                        table: table.value(),
                        column: column.value(),
                    });
                }

                "on_delete" => {
                    let value = meta.value()?;
                    let lit: LitStr = value.parse()?;
                    modifiers.push(Modifier::OnDelete(lit.value()));
                }

                // ── Validation rules ──
                "len" => {
                    // len(min = N, max = N)
                    let content;
                    syn::parenthesized!(content in meta.input);
                    let mut min_val: Option<u32> = None;
                    let mut max_val: Option<u32> = None;
                    while !content.is_empty() {
                        let key: syn::Ident = content.parse()?;
                        content.parse::<syn::Token![=]>()?;
                        let lit: LitInt = content.parse()?;
                        let n: u32 = lit.base10_parse()?;
                        match key.to_string().as_str() {
                            "min" => min_val = Some(n),
                            "max" => max_val = Some(n),
                            other => return Err(syn::Error::new_spanned(
                                &key,
                                format!("unknown len param `{}`. Expected: min, max", other),
                            )),
                        }
                        if content.peek(syn::Token![,]) {
                            content.parse::<syn::Token![,]>()?;
                        }
                    }
                    validations.push(ValidationRule::Length { min: min_val, max: max_val });
                }

                "email" => {
                    validations.push(ValidationRule::Email);
                }

                "url" => {
                    validations.push(ValidationRule::Url);
                }

                "range" => {
                    // range(min = N, max = N)
                    let content;
                    syn::parenthesized!(content in meta.input);
                    let mut min_val: Option<f64> = None;
                    let mut max_val: Option<f64> = None;
                    while !content.is_empty() {
                        let key: syn::Ident = content.parse()?;
                        content.parse::<syn::Token![=]>()?;
                        // Accept both integer and float literals
                        let n: f64 = if content.peek(LitFloat) {
                            let lit: LitFloat = content.parse()?;
                            lit.base10_parse()?
                        } else {
                            let lit: LitInt = content.parse()?;
                            lit.base10_parse::<i64>()? as f64
                        };
                        match key.to_string().as_str() {
                            "min" => min_val = Some(n),
                            "max" => max_val = Some(n),
                            other => return Err(syn::Error::new_spanned(
                                &key,
                                format!("unknown range param `{}`. Expected: min, max", other),
                            )),
                        }
                        if content.peek(syn::Token![,]) {
                            content.parse::<syn::Token![,]>()?;
                        }
                    }
                    validations.push(ValidationRule::Range { min: min_val, max: max_val });
                }

                "regex" => {
                    let content;
                    syn::parenthesized!(content in meta.input);
                    let lit: LitStr = content.parse()?;
                    validations.push(ValidationRule::Regex(lit.value()));
                }

                "required" => {
                    validations.push(ValidationRule::Required);
                }

                other => {
                    return Err(meta.error(format!(
                        "unknown #[col] attribute `{}`\n\
                         available: key, auto, unique, index, now, default, \
                         name, max, precision, scale, references, on_delete, \
                         len, email, url, range, regex, required",
                        other
                    )));
                }
            }

            Ok(())
        })?;
    }

    Ok(ColParseResult { modifiers, validations })
}

/// Extract `#[col(name = "...")]` if present.
pub(crate) fn extract_col_name(modifiers: &[Modifier]) -> Option<String> {
    for m in modifiers {
        if let Modifier::Default(val) = m {
            if let Some(name) = val.strip_prefix("__col_name__") {
                return Some(name.to_string());
            }
        }
    }
    None
}

/// Apply type overrides from `#[col(max = N)]`, `#[col(precision = N, scale = N)]`.
pub(crate) fn apply_type_overrides(mut type_info: TypeInfo, modifiers: &[Modifier]) -> TypeInfo {
    let mut max_override = None;
    let mut precision_override = None;
    let mut scale_override = None;

    for m in modifiers {
        if let Modifier::Default(val) = m {
            if let Some(n) = val.strip_prefix("__col_max__") {
                max_override = Some(n.parse::<u32>().unwrap_or(255));
            }
            if let Some(n) = val.strip_prefix("__col_precision__") {
                precision_override = Some(n.parse::<u32>().unwrap_or(10));
            }
            if let Some(n) = val.strip_prefix("__col_scale__") {
                scale_override = Some(n.parse::<u32>().unwrap_or(2));
            }
        }
    }

    if let Some(max) = max_override {
        if let TypeInfo::Varchar { .. } = type_info {
            type_info = TypeInfo::Varchar { max_length: max };
        }
    }

    if precision_override.is_some() || scale_override.is_some() {
        if let TypeInfo::Decimal { precision, scale } = type_info {
            type_info = TypeInfo::Decimal {
                precision: precision_override.unwrap_or(precision),
                scale: scale_override.unwrap_or(scale),
            };
        }
    }

    type_info
}
