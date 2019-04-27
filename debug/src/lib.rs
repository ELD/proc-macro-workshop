extern crate proc_macro;

use proc_macro::TokenStream;

use proc_quote::quote;

use syn::{DeriveInput, parse_macro_input::parse, parse_quote};

use std::collections::{HashSet, HashMap};

#[proc_macro_derive(CustomDebug, attributes(debug))]
pub fn derive(input: TokenStream) -> TokenStream {
    let mut input = parse::<DeriveInput>(input).unwrap();

    let input_ident = &input.ident;
    let input_name = format!("{}", input_ident);
    let generics = &mut input.generics;

    let named_fields = match input.data {
        syn::Data::Struct(s) => match s.fields {
            syn::Fields::Named(named_fields) => named_fields,
            _ => unimplemented!(),
        },
        _ => unimplemented!()
    };

    let phantom_data = collect_phantom_data(&named_fields);
    let attribute_fields = match collect_fields_format(&named_fields) {
        Ok(result) => result,
        Err(e) => return e.into(),
    };
    let debug_fields = format_debug_fields(&named_fields, &attribute_fields);
    let associated_types = collect_associated_types(&named_fields);
    let handwritten_type = collect_custom_bound_attr(&input.attrs);

    generics.type_params_mut()
        .into_iter()
        .for_each(|ty_param| {
            if !phantom_data.contains(&ty_param.ident) &&
                associated_types.get(&ty_param.ident).is_none() &&
                handwritten_type.is_none() {
                ty_param.bounds.push(parse_quote!(std::fmt::Debug))
            }
        });

    associated_types
        .iter()
        .for_each(|(_, assoc_ty)|
            generics.make_where_clause().predicates.push(parse_quote!(#assoc_ty: std::fmt::Debug))
        );

    if let Some(custom_bound) = handwritten_type {
        generics.make_where_clause().predicates.push(custom_bound);
    }

    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();

    let tokens = quote! {
        impl#impl_generics std::fmt::Debug for #input_ident#type_generics #where_clause {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.debug_struct(#input_name)
                    #debug_fields
                    .finish()
            }
        }
    };

    tokens.into()
}

fn format_debug_fields(named_fields: &syn::FieldsNamed, attribute_fields: &HashMap<syn::Ident, syn::Lit>) -> proc_macro2::TokenStream {
    let field_expansions = named_fields.named.iter()
        .map(|f| {
            let ident = f.ident.clone().unwrap();
            let ident_string = ident.to_string();

            match attribute_fields.get(&ident) {
                Some(literal) => quote! { .field(#ident_string, &format_args!(#literal, &self.#ident)) },
                None => quote! { .field(#ident_string, &self.#ident) },
            }
        });

    quote! { #(#field_expansions)* }
}

fn collect_phantom_data(named_fields: &syn::FieldsNamed) -> HashSet<syn::Ident> {
    named_fields.named
        .iter()
        .filter_map(|f| {
            let segment = match &f.ty {
                syn::Type::Path(ty) => &ty.path.segments[0],
                _ => return None,
            };

            if segment.ident != "PhantomData" {
                return None;
            }

            let argument = match &segment.arguments {
                syn::PathArguments::AngleBracketed(bracketed) => &bracketed.args[0],
                _ => return None,
            };

            match argument {
                syn::GenericArgument::Type(syn::Type::Path(arg)) => Some(arg.path.segments[0].ident.clone()),
                _ => None,
            }
        })
        .collect()
}

fn collect_fields_format(fields: &syn::FieldsNamed) -> Result<HashMap<syn::Ident, syn::Lit>, proc_macro2::TokenStream> {
    fields.named
        .iter()
        .filter_map(|f| {
            let attr = f.attrs.iter().find(|a| a.path.is_ident("debug"))?;
            let ident = f.clone().ident.unwrap();
            Some(match attr.parse_meta() {
                Ok(syn::Meta::NameValue(nv)) => Ok((ident, nv.lit)),
                Ok(_) => Err(syn::Error::new_spanned(attr, "attribute should be in the format of a name value").to_compile_error()),
                Err(e) => Err(e.to_compile_error()),
            })
        })
        .collect()
}

fn collect_associated_types(fields: &syn::FieldsNamed) -> HashMap<syn::Ident, syn::TypePath> {
    fields.named
        .iter()
        .filter_map(|f| {
            let segment = match &f.ty {
                syn::Type::Path(ty) => &ty.path.segments[0],
                _ => return None,
            };

            let argument = match &segment.arguments {
                syn::PathArguments::AngleBracketed(bracketed) => &bracketed.args[0],
                _ => return None,
            };

            let generic_type_path = match &argument {
                syn::GenericArgument::Type(syn::Type::Path(type_path)) => type_path,
                _ => return None,
            };

            if generic_type_path.path.segments.len() < 2 {
                return None;
            }

            Some((generic_type_path.path.segments[0].ident.clone(), generic_type_path.clone()))
        })
        .collect()
}

fn collect_custom_bound_attr(input_attr: &[syn::Attribute]) -> Option<syn::WherePredicate> {
    let attr = input_attr.iter()
        .filter_map(|a| a.parse_meta().ok())
        .nth(0);

    if let Some(meta) = attr {
        match meta {
            syn::Meta::List(meta_list) => {
                match &meta_list.nested[0] {
                    syn::NestedMeta::Meta(syn::Meta::NameValue(name_value)) => {
                        match &name_value.lit {
                            syn::Lit::Str(lit_str) => {
                                return lit_str.parse().ok();
                            }
                            _ => return None,
                        }
                    }
                    _ => return None,
                }
            }
            _ => return None,
        }
    }

    None
}
