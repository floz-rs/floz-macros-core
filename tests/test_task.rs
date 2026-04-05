use floz_macros_core::task::TaskAttr;
use quote::quote;

#[test]
fn test_task_attr_parser_full() {
    let attr_tokens = quote! { name = "send_email", queue = "critical", retries = 5, timeout = 60 };
    let attr = syn::parse2::<TaskAttr>(attr_tokens).unwrap();
    
    assert_eq!(attr.name, Some("send_email".to_string()));
    assert_eq!(attr.queue, Some("critical".to_string()));
    assert_eq!(attr.retries, Some(5));
    assert_eq!(attr.timeout, Some(60));
}

#[test]
fn test_task_attr_parser_partial() {
    let attr_tokens = quote! { queue = "default" };
    let attr = syn::parse2::<TaskAttr>(attr_tokens).unwrap();
    
    assert_eq!(attr.name, None);
    assert_eq!(attr.queue, Some("default".to_string()));
    assert_eq!(attr.retries, None);
    assert_eq!(attr.timeout, None);
}

#[test]
fn test_task_attr_parser_invalid() {
    let attr_tokens = quote! { invalid_key = "value" };
    let err = syn::parse2::<TaskAttr>(attr_tokens).unwrap_err();
    assert!(err.to_string().contains("unknown task attribute `invalid_key`"));
}
