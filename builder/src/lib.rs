extern crate proc_macro;

use proc_macro::TokenStream;

use syn::{parse_macro_input::parse, DeriveInput, Ident, Data::Struct, Fields, Type};
use proc_quote::quote;
use syn::export::Span;

/// # Summary of procedural macro:
/// Three cases to contend with:
/// 1. Normal, named fields
/// 1. Normal, named fields with optional types (i.e. `field: Option<String>`)
/// 1. Normal, named fields with attribute builders (i.e. `#[builder(each = "var")] vars: Vec<String>`)
///
/// Example:
/// ```rust,ignore
/// #[derive(Builder)]
/// pub struct Command {
///    executable: String, // Case 1
///    #[builder(each = "arg")] // Case 3
///    args: Vec<String>,
///    env: Vec<String>,
///    current_dir: Option<String>, // Case 2
/// }
/// ```
///
/// ## Case 1:
/// This is the simplest case. Just take the ident and type from the field and
/// dump it into a `TokenStream` output.
///
/// This is considered a _required field_.
///
/// ex:
/// ```rust,ignore
/// ...
/// command: String,
/// ...
/// // converts to
/// ...
/// command: Option<String>,
/// ...
/// // with setter
/// ...
/// fn command(&mut self, command: String) -> &mut Self {
///     self.command = Some(command);
///     self
/// }
/// ```
///
/// ## Case 2:
/// This one is a bit trickier. It requires lifting the inner type from the `Option<..>` type
/// and then dumping the ident and type into a `TokenStream` output.
///
/// This can be considered an _optional_ field.
///
/// ex:
/// ```rust,ignore
/// ...
/// directory: Option<String>,
/// ...
/// // Converts to
/// ...
/// directory: Option<String>
/// ...
/// // with setter:
/// fn directory(&mut self, directory: String) -> &mut Self {
///     self.directory = Some(directory);
///     self
/// }
/// ```
///
/// ## Case 3:
/// This is the hardest of all three cases. This involves reaching into the attributes on the field
/// to get the name of the builder. From there, lifting the type is required from the collection type.
/// Once that's been done, the singularized ident and lifted type need to be dumped into a
/// `TokenStream` output.
///
/// This can be considered a _builder_field_.
///
/// ex:
/// ```rust,ignore
/// ...
/// #[builder(each = "arg")]
/// args: Vec<String>,
/// ...
/// // converts to
/// ...
/// args: Vec<String>
/// ...
/// // with a setter:
/// fn arg(&mut self, arg: String) -> &mut Self {
///     self.args.push(arg);
///     self
/// }
/// ```
///
/// ## Building the builder struct
/// Once all of these fields have been identified, the Builder struct has been generated and the
/// setters have been generated, it's time to generate the `build` method.
///
/// For each case:
///
/// 1. Check if the type is `None`. If it is, return an error.
/// 1. Pass through the type, skip a `None` check because the field is optional in the original struct.
/// 1. Pass through the type, skip a `None` check. It's an aggregate type, so an empty collction
/// is still a valid value.
///
#[proc_macro_derive(Builder, attributes(builder))]
pub fn derive(input: TokenStream) -> TokenStream {
    let derive_input = parse::<DeriveInput>(input).unwrap();
    let vis = &derive_input.vis;
    let input_ident = &derive_input.ident;
    let builder_ident = Ident::new(&format!("{}Builder", input_ident), Span::call_site());

    let (builder_initializer, builder_fields, builder_setters, builder_build_fn) = match derive_input.data {
        Struct(s) => {
            match s.fields {
                Fields::Named(fields) => {
                    let optional_fields = fetch_optional_fields(&fields);
                    let builder_fields = match fetch_builder_fields(&fields) {
                        Some(f) => f,
                        None => {
                            return TokenStream::from(quote! {
                                compile_error!("missing or unrecognized attribute in `builder`");
                            });
                        }
                    };
                    (
                        expand_builder_initializer(&fields, &builder_fields),
                        expand_builder_fields(&fields, &optional_fields, &builder_fields),
                        expand_builder_setters(&fields, &optional_fields, &builder_fields),
                        expand_builder_build_fn(&input_ident, &fields, &optional_fields, &builder_fields),
                    )
                }
                _ => { (quote!(), quote!(), quote!(), quote!()) }
            }
        }
        _ => { unimplemented!() }
    };

    let tokens = quote! {
        impl #input_ident {
            #vis fn builder() -> #builder_ident {
                #builder_ident {
                    #builder_initializer
                }
            }
        }

        #vis struct #builder_ident {
            #builder_fields
        }

        impl #builder_ident {
            #builder_setters

            #builder_build_fn
        }
    };

    tokens.into()
}

fn fetch_optional_fields(fields: &syn::FieldsNamed) -> Vec<syn::Field> {
    fields.named.iter()
        .cloned()
        .filter(|f| match f.ty {
            Type::Path(ref ty_path) => {
                ty_path.path.segments[0].ident == "Option"
            }
            _ => unimplemented!(),
        })
        .collect()
}

fn fetch_builder_fields(fields: &syn::FieldsNamed) -> Option<Vec<syn::Field>> {
    let fields = fields.named.iter()
        .cloned()
        .filter(|f| f.attrs.len() > 0 && f.attrs.iter().any(|attr| attr.path.segments[0].ident == "builder"))
        .collect::<Vec<syn::Field>>();

    let filtered_fields = fields.iter().cloned().filter(|field| {
        field.attrs.iter().all(|attr| {
            if let Ok(parsed_meta) = attr.parse_meta() {
                let attr_ident = match parsed_meta {
                    syn::Meta::List(meta_list) => {
                        match &meta_list.nested[0] {
                            syn::NestedMeta::Meta(syn::Meta::NameValue(name_value)) => {
                                name_value.ident.clone()
                            }
                            _ => unreachable!(),
                        }
                    }
                    _ => unreachable!(),
                };

                if attr_ident != "each" { return false; }
            }

            true
        })
    }).collect::<Vec<syn::Field>>();

    if fields.len() != filtered_fields.len() {
        return None;
    }

    Some(fields)
}

fn expand_builder_initializer(fields: &syn::FieldsNamed, builder_fields: &[syn::Field]) -> proc_macro2::TokenStream {
    let initializers = fields.named.iter().map(|f| {
        let ident = &f.ident.clone().unwrap();

        if builder_fields.contains(f) {
            let ty = &f.ty;
            quote! {
                #ident: <#ty>::new(),
            }
        } else {
            quote! {
                #ident: ::std::option::Option::None,
            }
        }
    });

    quote! { #(#initializers)* }
}

fn expand_builder_fields(fields: &syn::FieldsNamed, optional_fields: &[syn::Field], builder_fields: &[syn::Field]) -> proc_macro2::TokenStream {
    let expanded_fields = fields.named.iter()
        .map(|f| {
            let ident = &f.ident.clone().unwrap();
            let ty = &f.ty;

            if optional_fields.contains(f) || builder_fields.contains(f) {
                quote! {
                    #ident: #ty,
                }
            } else {
                quote! {
                    #ident: ::std::option::Option<#ty>,
                }
            }
        });

    quote! { #(#expanded_fields)* }
}

fn expand_builder_setters(fields: &syn::FieldsNamed, optional_fields: &[syn::Field], builder_fields: &[syn::Field]) -> proc_macro2::TokenStream {
    let setters = fields.named.iter().map(|f| {
        if optional_fields.contains(f) {
            let ident = &f.ident.clone().unwrap();
            let ty = get_inner_type(&f);
            quote! {
                fn #ident(&mut self, #ident: #ty) -> &mut Self {
                    self.#ident = ::std::option::Option::Some(#ident);
                    self
                }
            }
        } else if builder_fields.contains(f) {
            let ident = f.ident.clone().unwrap();
            let attr_ident = get_attr_ident(f); // TODO: Get the ident from the attr
            let ty = get_inner_type(&f);
            quote! {
                fn #attr_ident(&mut self, #ident: #ty) -> &mut Self {
                    self.#ident.push(#ident);
                    self
                }
            }
        } else {
            let ident = &f.ident.clone().unwrap();
            let attr_ident = &f.ident.clone().unwrap();
            let ty = &f.ty;

            quote! {
                fn #attr_ident(&mut self, #ident: #ty) -> &mut Self {
                    self.#ident = ::std::option::Option::Some(#ident);
                    self
                }
            }
        }
    });

    quote! { #(#setters)* }
}

fn expand_builder_build_fn(input_ident: &Ident, fields: &syn::FieldsNamed, optional_fields: &[syn::Field], builder_fields: &[syn::Field]) -> proc_macro2::TokenStream {
    let required_fields_none_check = fields.named.iter().map(|f| {
        if !optional_fields.contains(f) && !builder_fields.contains(f) {
            let ident = &f.ident.clone().unwrap();
            let ident_str = ident.to_string();
            quote! {
                if self.#ident.is_none() {
                    return ::std::result::Result::Err(::std::boxed::Box::from(format!("value is not set: {}", #ident_str)));
                }
            }
        } else {
            quote! {}
        }
    });

    let field_assignment = fields.named.iter().map(|f| {
        let ident = &f.ident.clone().unwrap();

        if optional_fields.contains(f) {
            quote! {
                #ident: self.#ident.take(),
            }
        } else if builder_fields.contains(f) {
            quote! {
                #ident: self.#ident.drain(..).collect(),
            }
        } else {
            quote! {
                #ident: self.#ident.take().unwrap(),
            }
        }
    });

    quote! {
        fn build(&mut self) -> ::std::result::Result<#input_ident, ::std::boxed::Box<dyn std::error::Error>> {
            #(#required_fields_none_check)*
            ::std::result::Result::Ok(#input_ident {
                #(#field_assignment)*
            })
        }
    }
}

fn get_inner_type(field: &syn::Field) -> &syn::Type {
    match field.ty {
        Type::Path(ref type_path) => {
            match type_path.path.segments[0].arguments {
                syn::PathArguments::AngleBracketed(ref angle_bracketed) => {
                    match angle_bracketed.args[0] {
                        syn::GenericArgument::Type(ref arg_type) => arg_type,
                        _ => unreachable!(),
                    }
                }
                _ => unreachable!()
            }
        }
        _ => unreachable!()
    }
}

fn get_attr_ident(field: &syn::Field) -> syn::Ident {
    if field.attrs.len() > 0 {
        let attr = field.attrs
            .iter()
            .filter_map(|a| a.parse_meta().ok())
            .nth(0);

        let attr_str = match attr {
            Some(syn::Meta::List(meta_list)) => {
                match &meta_list.nested[0] {
                    syn::NestedMeta::Meta(syn::Meta::NameValue(name_value)) => {
                        match &name_value.lit {
                            syn::Lit::Str(lit_str) => lit_str.clone(),
                            _ => unreachable!(),
                        }
                    }
                    _ => unreachable!(),
                }
            }
            _ => unreachable!(),
        };

        syn::Ident::new(&attr_str.value(), attr_str.span())
    } else {
        field.clone().ident.unwrap()
    }
}
