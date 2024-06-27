use proc_macro::TokenStream;
use syn::{parse_macro_input, Ident, LitInt};

#[proc_macro]
pub fn seq(input: TokenStream) -> TokenStream {
    let _ = input;

    let st = parse_macro_input!(input as SeqParser);

    TokenStream::new()
}

struct SeqParser {
    variable_ident: syn::Ident,
    start:          isize,
    end:            isize,
    body:           proc_macro2::TokenStream,
}

impl syn::parse::Parse for SeqParser {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let variable_ident = input.parse::<Ident>()?;

        input.parse::<syn::token::In>()?;

        let start = input.parse::<LitInt>()?;

        input.parse::<syn::token::DotDot>()?;

        let end = input.parse::<LitInt>()?;

        let body_buf;

        syn::braced!(body_buf in input);

        let body = body_buf.parse::<proc_macro2::TokenStream>()?;

        Ok(SeqParser {
            variable_ident,
            start: start.base10_parse()?,
            end: end.base10_parse()?,
            body,
        })
    }
}
