use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::spanned::Spanned;
use syn::{Data, DataStruct, DeriveInput, Fields, FieldsNamed, LitStr};

pub fn expand_builder(input: DeriveInput) -> syn::Result<TokenStream> {
    let struct_ident = &input.ident;
    let builder_name_ident = format_ident!("{}Builder", struct_ident);

    let fields = get_fields_from_derive_input(&input)?;
    let builder_struct_fields_def = generate_builder_struct_fields_def(fields)?;
    let builder_struct_factory_init_clauses = generate_builder_struct_factory_init_clauses(fields)?;
    let setter_functions = generate_setter_functions(fields)?;
    let generated_builder_functions = generate_build_function(fields, struct_ident)?;

    let ret = quote! {
        #[automatically_derived]
        pub struct #builder_name_ident {
            #builder_struct_fields_def
        }

        #[automatically_derived]
        impl #struct_ident {
            pub fn builder()->#builder_name_ident {
                #builder_name_ident {
                    #(#builder_struct_factory_init_clauses),*
                }
            }
        }

        #[automatically_derived]
        impl #builder_name_ident {
            #setter_functions
            #generated_builder_functions
        }
    };

    Ok(ret)
}

type StructFields = syn::punctuated::Punctuated<syn::Field, syn::token::Comma>;

fn get_fields_from_derive_input(d: &syn::DeriveInput) -> syn::Result<&StructFields> {
    let Data::Struct(DataStruct {
        fields: Fields::Named(FieldsNamed { ref named, .. }),
        ..
    }) = d.data
    else {
        return Err(syn::Error::new_spanned(
            d,
            "this derive macro only works on structs with named fields",
        ));
    };
    Ok(named)
}

fn generate_builder_struct_fields_def(fields: &StructFields) -> syn::Result<TokenStream> {
    let builder_fields = fields
        .iter()
        .map(|f| {
            let ident = &f.ident;
            let stmt = if let Some(inner_ty) = get_generic_inner_type(&f.ty, "Option") {
                quote! {#ident: std::option::Option<#inner_ty>}
            } else if get_user_specified_ident_for_vec(f)?.is_some() {
                let ty = &f.ty;
                quote! {#ident: #ty}
            } else {
                let ty = &f.ty;
                quote! {#ident: std::option::Option<#ty>}
            };
            Ok(stmt)
        })
        .collect::<syn::Result<Vec<_>>>()?;
    Ok(quote! {
        #(#builder_fields),*
    })
}

fn generate_builder_struct_factory_init_clauses(
    fields: &StructFields,
) -> syn::Result<Vec<TokenStream>> {
    let init_clauses = fields
        .iter()
        .map(|f| {
            let ident = &f.ident;
            let stmt = if get_user_specified_ident_for_vec(f)?.is_some() {
                quote! {
                    #ident: std::vec::Vec::new()
                }
            } else {
                quote! {
                    #ident: std::option::Option::None
                }
            };
            Ok(stmt)
        })
        .collect::<syn::Result<Vec<_>>>()?;
    Ok(init_clauses)
}

fn generate_setter_functions(fields: &StructFields) -> syn::Result<TokenStream> {
    let mut final_tokenstream = quote! {};

    for f in fields.iter() {
        let ident = &f.ident;
        let ty = &f.ty;
        let toeknstream_piece = if let Some(inner_ty) = get_generic_inner_type(ty, "Option") {
            quote! {
                fn #ident(&mut self, #ident: #inner_ty) -> &mut Self {
                    self.#ident = std::option::Option::Some(#ident);
                    self
                }
            }
        } else if let Some(ref user_specified_ident) = get_user_specified_ident_for_vec(f)? {
            let inner_ty = get_generic_inner_type(ty, "Vec").ok_or(syn::Error::new(
                f.span(),
                "each field must be specified with Vec field",
            ))?;
            let mut tokenstream = quote! {
                fn #user_specified_ident(&mut self, #user_specified_ident: #inner_ty) -> &mut Self{
                    self.#ident.push(#user_specified_ident);
                    self
                }
            };
            if user_specified_ident != ident.as_ref().unwrap() {
                tokenstream.extend(quote! {
                    fn #ident(&mut self, #ident: #ty) -> &mut Self {
                        self.#ident = #ident.clone();
                        self
                    }
                })
            }
            tokenstream
        } else {
            quote! {
                fn #ident(&mut self, #ident: #ty) -> &mut Self{
                    self.#ident = std::option::Option::Some(#ident);
                    self
                }
            }
        };
        final_tokenstream.extend(toeknstream_piece);
    }

    Ok(final_tokenstream)
}

fn generate_build_function(
    fields: &StructFields,
    origin_struct_ident: &syn::Ident,
) -> syn::Result<TokenStream> {
    let checker_code_pieces = fields
        .iter()
        .map(|f| {
            let ident = &f.ident;
            let ty = &f.ty;
            let stmt = if get_optional_inner_type(ty).is_none()
                && get_user_specified_ident_for_vec(f)?.is_none()
            {
                quote! {
                    if self.#ident.is_none(){
                        let err = format!("{} field missing", stringify!(#ident));
                        return std::result::Result::Err(err.into())
                    }
                }
            } else {
                quote! {}
            };
            // syn::Result::Ok(stmt)
            Ok(stmt)
        })
        .collect::<syn::Result<Vec<_>>>()?;

    let fill_result_clauses = fields
        .iter()
        .map(|f| {
            let ident = &f.ident;
            let ty = &f.ty;
            let stmt = if get_user_specified_ident_for_vec(f)?.is_some() {
                quote! {
                    #ident: self.#ident.clone()
                }
            } else if get_optional_inner_type(ty).is_none() {
                quote! {
                    #ident: self.#ident.clone().unwrap()
                }
            } else {
                quote! {
                    #ident: self.#ident.clone()
                }
            };
            Ok(stmt)
        })
        .collect::<syn::Result<Vec<_>>>()?;

    let token_stream = quote! {
        pub fn build(&mut self) -> ::std::result::Result<#origin_struct_ident,std::boxed::Box<dyn std::error::Error>> {
            #(#checker_code_pieces)*

            let ret = #origin_struct_ident {
                #(#fill_result_clauses),*
            };
            std::result::Result::Ok(ret)
        }
    };
    Ok(token_stream)
}

fn get_optional_inner_type(ty: &syn::Type) -> Option<&syn::Type> {
    if let syn::Type::Path(syn::TypePath { ref path, .. }) = ty {
        // 这里我们取segments的最后一节来判断是不是`Option<T>`，这样如果用户写的是`std:option:Option<T>`我们也能识别出最后的`Option<T>`
        if let Some(seg) = path.segments.last() {
            if seg.ident == "Option" {
                if let syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
                    ref args,
                    ..
                }) = seg.arguments
                {
                    if let Some(syn::GenericArgument::Type(inner_ty)) = args.first() {
                        return Some(inner_ty);
                    }
                }
            }
        }
    }
    None
}

fn get_generic_inner_type<'a>(ty: &'a syn::Type, outer_ident_name: &str) -> Option<&'a syn::Type> {
    if let syn::Type::Path(syn::TypePath { path, .. }) = ty {
        if let Some(seg) = path.segments.last() {
            if seg.ident == outer_ident_name {
                if let syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
                    ref args,
                    ..
                }) = seg.arguments
                {
                    if let Some(syn::GenericArgument::Type(inner_ty)) = args.first() {
                        return Some(inner_ty);
                    }
                }
            }
        }
    }
    None
}

fn get_user_specified_ident_for_vec(field: &syn::Field) -> syn::Result<Option<syn::Ident>> {
    let mut id = None;
    for attr in field.attrs.iter().filter(|a| a.path().is_ident("builder")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("each") {
                let s: LitStr = meta.value()?.parse()?;
                id = Some(syn::Ident::new(&s.value(), s.span()));
            } else {
                return Err(syn::Error::new_spanned(
                    &attr.meta,
                    r#"expected `builder(each = "...")`"#,
                ));
            }
            Ok(())
        })?;
    }
    Ok(id)
}
