use proc_macro2::{Span, TokenStream};
use syn::{parse2, LitStr};
use crate::crud::{self, CrudConfig};

pub(crate) struct ModelAttr {
    pub table_name: String,
    pub crud_config: Option<CrudConfig>,
    pub soft_delete: bool,
}

impl syn::parse::Parse for ModelAttr {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        // First token must be the table name string
        let table_lit: LitStr = input.parse().map_err(|_| {
            syn::Error::new(
                Span::call_site(),
                "#[model] requires a table name, e.g. #[model(\"notes\")]",
            )
        })?;

        let mut crud_config = None;
        let mut soft_delete = false;

        // Loop over remaining tokens separated by commas
        while input.peek(syn::Token![,]) {
            input.parse::<syn::Token![,]>()?;

            if !input.is_empty() {
                let ident: syn::Ident = input.parse()?;
                if ident == "crud" {
                    if input.peek(syn::token::Paren) {
                        // crud(tag = "...", ...)
                        let content;
                        syn::parenthesized!(content in input);
                        crud_config = Some(parse_crud_options(&content)?);
                    } else {
                        // Plain `crud` with no options
                        crud_config = Some(CrudConfig::default());
                    }
                } else if ident == "soft_delete" {
                    soft_delete = true;
                } else {
                    return Err(syn::Error::new_spanned(
                        &ident,
                        format!(
                            "unknown #[model] option `{}`. Available: crud, soft_delete",
                            ident
                        ),
                    ));
                }
            }
        }

        Ok(ModelAttr {
            table_name: table_lit.value(),
            crud_config,
            soft_delete,
        })
    }
}

/// Parse the inner options of `crud(tag = "...", path = "...", ...)`.
fn parse_crud_options(input: syn::parse::ParseStream) -> syn::Result<CrudConfig> {
    let mut config = CrudConfig::default();

    while !input.is_empty() {
        let key: syn::Ident = input.parse()?;

        match key.to_string().as_str() {
            "tag" => {
                input.parse::<syn::Token![=]>()?;
                let lit: LitStr = input.parse()?;
                config.tag = Some(lit.value());
            }
            "path" => {
                input.parse::<syn::Token![=]>()?;
                let lit: LitStr = input.parse()?;
                config.path = Some(lit.value());
            }
            "auth" => {
                input.parse::<syn::Token![=]>()?;
                let lit: LitStr = input.parse()?;
                config.auth = Some(lit.value());
            }
            "only" => {
                let content;
                syn::parenthesized!(content in input);
                let mut ops = Vec::new();
                while !content.is_empty() {
                    let op_ident: syn::Ident = content.parse()?;
                    match crud::CrudOp::from_str(&op_ident.to_string()) {
                        Some(op) => ops.push(op),
                        None => return Err(syn::Error::new_spanned(
                            &op_ident,
                            format!(
                                "unknown CRUD operation `{}`. Available: list, create, get, update, delete",
                                op_ident
                            ),
                        )),
                    }
                    if content.peek(syn::Token![,]) {
                        content.parse::<syn::Token![,]>()?;
                    }
                }
                config.only = Some(ops);
            }
            "exclude" => {
                let content;
                syn::parenthesized!(content in input);
                let mut ops = Vec::new();
                while !content.is_empty() {
                    let op_ident: syn::Ident = content.parse()?;
                    match crud::CrudOp::from_str(&op_ident.to_string()) {
                        Some(op) => ops.push(op),
                        None => return Err(syn::Error::new_spanned(
                            &op_ident,
                            format!(
                                "unknown CRUD operation `{}`. Available: list, create, get, update, delete",
                                op_ident
                            ),
                        )),
                    }
                    if content.peek(syn::Token![,]) {
                        content.parse::<syn::Token![,]>()?;
                    }
                }
                config.exclude = Some(ops);
            }
            other => {
                return Err(syn::Error::new_spanned(
                    &key,
                    format!(
                        "unknown crud option `{}`. Available: tag, path, auth, only, exclude",
                        other
                    ),
                ));
            }
        }

        if input.peek(syn::Token![,]) {
            input.parse::<syn::Token![,]>()?;
        }
    }

    Ok(config)
}

/// Parse `#[model("table")]` or `#[model("table", crud)]` or `#[model("table", soft_delete)]`.
pub(crate) fn parse_model_attr(attr: TokenStream) -> syn::Result<(String, Option<CrudConfig>, bool)> {
    let parsed: ModelAttr = parse2(attr)?;
    Ok((parsed.table_name, parsed.crud_config, parsed.soft_delete))
}
