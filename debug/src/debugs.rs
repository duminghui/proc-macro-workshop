use std::collections::HashMap;

use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::parse::Parser;
use syn::visit::{self, Visit};
use syn::{
    parse_quote, parse_str, Data, DataStruct, DeriveInput, Fields, FieldsNamed, LitStr, Path,
};

type StructFields = syn::punctuated::Punctuated<syn::Field, syn::token::Comma>;

pub fn expand_debug(input: DeriveInput) -> syn::Result<TokenStream> {
    generate_debug_trait(&input)
}

fn generate_debug_trait(input: &DeriveInput) -> syn::Result<TokenStream> {
    let fmt_body_stream = generate_debug_trait_core(input)?;

    let struct_name_ident = &input.ident;

    let fields = get_fields_from_derive_input(input)?;
    let mut field_type_names = vec![];
    let mut phantomdata_type_param_names = vec![];

    for field in fields {
        if let Some(s) = get_field_type_name(field)? {
            field_type_names.push(s);
        }
        if let Some(s) = get_phantomdata_generic_type_name(field)? {
            phantomdata_type_param_names.push(s);
        }
    }

    let mut generics_param_to_modify = input.generics.clone();

    if let Some(hatch) = get_struct_escape_hatch(input)? {
        let predicates = &mut generics_param_to_modify.make_where_clause().predicates;
        predicates.push(parse_str(hatch.as_str())?)
    } else {
        let associated_types_map = get_generic_associated_types(input);

        for g in generics_param_to_modify.params.iter_mut() {
            if let syn::GenericParam::Type(t) = g {
                let type_param_name = &t.ident.to_string();
                // 如果是PhantomData，就不要对泛型参数`T`本身再添加约束了,除非`T`本身也被直接使用了
                if phantomdata_type_param_names.contains(type_param_name)
                    && !field_type_names.contains(type_param_name)
                {
                    continue;
                }

                if associated_types_map.contains_key(type_param_name)
                    && !field_type_names.contains(type_param_name)
                {
                    continue;
                }

                t.bounds.push(parse_quote!(std::fmt::Debug));
            }
        }
        let predicates = &mut generics_param_to_modify.make_where_clause().predicates;
        for (_, associated_types) in associated_types_map {
            for associated_type in associated_types {
                predicates.push(parse_quote!(#associated_type: std::fmt::Debug));
            }
        }
    }

    let (impl_generics, type_generics, where_clause) = generics_param_to_modify.split_for_impl();

    let ret_stream = quote! {
        #[automatically_derived]
        impl #impl_generics std::fmt::Debug for #struct_name_ident #type_generics #where_clause{
            fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
                #fmt_body_stream
            }
        }
    };
    Ok(ret_stream)
}

fn generate_debug_trait_core(input: &DeriveInput) -> syn::Result<TokenStream> {
    let fields = get_fields_from_derive_input(input)?;
    let struct_name_ident = &input.ident;
    let struct_name_literal = struct_name_ident.to_string();
    let mut fmt_body_stream = TokenStream::new();
    fmt_body_stream.extend(quote! {
        fmt.debug_struct(#struct_name_literal)
    });
    for field in fields.iter() {
        let field_name_ident = field.ident.as_ref().unwrap();
        let field_name_literal = field_name_ident.to_string();

        let format_str = if let Some(format) = get_custom_format_of_field(field)? {
            format
        } else {
            "{:?}".to_string()
        };

        fmt_body_stream.extend(quote! {
            .field(#field_name_literal, &format_args!(#format_str, &self.#field_name_ident))
        });
    }
    fmt_body_stream.extend(quote! {.finish()});
    Ok(fmt_body_stream)
}

fn get_fields_from_derive_input(d: &syn::DeriveInput) -> syn::Result<&StructFields> {
    let Data::Struct(DataStruct {
        fields: Fields::Named(FieldsNamed { ref named, .. }),
        ..
    }) = d.data
    else {
        return Err(syn::Error::new_spanned(d, "Must define on a Struct"));
    };
    Ok(named)
}

fn get_custom_format_of_field(field: &syn::Field) -> syn::Result<Option<String>> {
    let mut debug = None::<String>;

    for attr in field.attrs.iter() {
        syn::meta::parser(|meta| {
            if meta.path.is_ident("debug") {
                let s = meta.value()?.parse::<LitStr>()?;
                debug = Some(s.value());
            }
            Ok(())
        })
        .parse2(attr.meta.to_token_stream())?;
    }
    Ok(debug)
}

fn get_struct_escape_hatch(input: &syn::DeriveInput) -> syn::Result<Option<String>> {
    let mut bound = None::<String>;
    for attr in input.attrs.iter().filter(|a| a.path().is_ident("debug")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("bound") {
                let s = meta.value()?.parse::<LitStr>()?;
                bound = Some(s.value())
            }
            Ok(())
        })?;
    }
    Ok(bound)
}

// PhantomData<T> get T
fn get_phantomdata_generic_type_name(field: &syn::Field) -> syn::Result<Option<String>> {
    if let syn::Type::Path(syn::TypePath {
        path: Path { ref segments, .. },
        ..
    }) = field.ty
    {
        if let Some(syn::PathSegment { ident, arguments }) = segments.last() {
            if ident == "PhantomData" {
                // eprintln!("{:#?}", field.ty);
                if let syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
                    args,
                    ..
                }) = arguments
                {
                    if let Some(syn::GenericArgument::Type(syn::Type::Path(syn::TypePath {
                        path,
                        ..
                    }))) = args.first()
                    {
                        if let Some(generic_ident) = path.segments.first() {
                            return Ok(Some(generic_ident.ident.to_string()));
                        }
                    }
                }
            }
        }
    }
    Ok(None)
}

// foo: XXX foo: XXX<YYY> get XXX, exclude YYYY
fn get_field_type_name(field: &syn::Field) -> syn::Result<Option<String>> {
    if let syn::Type::Path(syn::TypePath {
        path: syn::Path { ref segments, .. },
        ..
    }) = field.ty
    {
        if let Some(syn::PathSegment { ref ident, .. }) = segments.last() {
            return Ok(Some(ident.to_string()));
        }
    }
    Ok(None)
}

struct TypePathVisitor {
    generic_type_names: Vec<String>,
    associated_types:   HashMap<String, Vec<syn::TypePath>>,
}

impl<'ast> Visit<'ast> for TypePathVisitor {
    fn visit_type_path(&mut self, node: &'ast syn::TypePath) {
        // eprintln!(
        //     "### {} ### {} ### {:#?}",
        //     node.path.to_token_stream(),
        //     node.path.segments.len(),
        //     node.path,
        // );
        if node.path.segments.len() >= 2 {
            let generic_type_name = node.path.segments[0].ident.to_string();
            if self.generic_type_names.contains(&generic_type_name) {
                self.associated_types
                    .entry(generic_type_name)
                    .or_default()
                    .push(node.clone())
            }
        }
        visit::visit_type_path(self, node);
    }
}

fn get_generic_associated_types(st: &syn::DeriveInput) -> HashMap<String, Vec<syn::TypePath>> {
    let origin_generic_param_names = st
        .generics
        .params
        .iter()
        .filter_map(|f| {
            if let syn::GenericParam::Type(ty) = f {
                Some(ty.ident.to_string())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    // println!("generic_param_names: {:?}", origin_generic_param_names);
    let mut visitor = TypePathVisitor {
        generic_type_names: origin_generic_param_names,
        associated_types:   HashMap::new(),
    };
    visitor.visit_derive_input(st);
    visitor.associated_types
}
