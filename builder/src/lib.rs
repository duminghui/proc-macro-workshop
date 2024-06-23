use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

mod builders;

#[proc_macro_derive(Builder, attributes(builder))]
pub fn derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    // let ty: syn::Type = parse_quote!(Vec<String>);
    // eprintln!("ty: {:?}", ty);
    builders::expand_builder(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
