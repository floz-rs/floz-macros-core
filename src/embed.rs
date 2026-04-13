use proc_macro2::TokenStream;
use quote::quote;
use std::env;
use std::fs;
use std::path::Path;

pub fn expand_embed_migrations() -> TokenStream {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let base_dir = Path::new(&manifest_dir).join("src").join("app");

    let mut tuples = Vec::new();

    if base_dir.exists() {
        if let Ok(entries) = fs::read_dir(&base_dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    let mig_dir = entry.path().join("migrations");
                    if mig_dir.exists() {
                        if let Ok(migs) = fs::read_dir(&mig_dir) {
                            for mig in migs.flatten() {
                                let name = mig.file_name().into_string().unwrap();
                                if name.starts_with('v') && name.ends_with(".json") {
                                    if let Ok(v) = name[1..name.len() - 5].parse::<i32>() {
                                        let file_path = mig.path().to_string_lossy().to_string();
                                        // We use include_str! to embed it strictly at compile time
                                        tuples.push(quote! {
                                            _migrations.insert(#v, include_str!(#file_path));
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    quote! {
        {
            let mut _migrations: std::collections::BTreeMap<i32, &'static str> = std::collections::BTreeMap::new();
            #(#tuples)*
            _migrations
        }
    }
}
