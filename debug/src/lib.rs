use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

mod debugs;

#[proc_macro_derive(CustomDebug, attributes(debug))]
pub fn derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    debugs::expand_debug(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
