extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input,
    Token,
    Error,
    parse::{
        ParseBuffer,
    },
    punctuated::Punctuated,
    ExprPath,
    Fields,
    Ident,
    Type,
};

mod bits;
use bits::GenerateBits;

#[proc_macro]
pub fn generate_bits(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as GenerateBits);
    let mut out = proc_macro2::TokenStream::new();

    (input.from..input.to).for_each(|idx| {
        let ident = Ident::new(&format!("B{}", idx), proc_macro2::Span::call_site());
        out.extend(quote! {
            pub enum #ident {}

            impl Specifier for #ident {
                const BITS: usize = #idx;
            }
        });
    });

    out.into()
}

#[proc_macro_attribute]
pub fn bitfield(args: TokenStream, input: TokenStream) -> TokenStream {
    let _ = args;
    let bitfield = parse_macro_input!(input as Bitfield);

    bitfield.expand().into()
}

struct Bitfield {
    ast: syn::ItemStruct,
}

impl Bitfield {
    fn expand(&self) -> proc_macro2::TokenStream {
        let mut sizes = Punctuated::<ExprPath, Token![+]>::new();
        let mut fields = Vec::new();
        match &self.ast.fields {
            Fields::Named(fields_named) => {
                for field in fields_named.named.iter() {
                    let ty = &field.ty;
                    fields.push((field.ident.clone().unwrap(), field.ty.clone()));
                    sizes.push(syn::parse_quote!(<#ty as Specifier>::BITS))
                }
            }
            _ => {}
        };
        let vis = &self.ast.vis;
        let ident = &self.ast.ident;
        let data_ident = Ident::new("data", proc_macro2::Span::call_site());

        let constructor = self.expand_constructor(&data_ident, &sizes);
        let accessors = self.expand_accessors(&fields);

        quote! {
            #[repr(C)]
            #vis struct #ident {
                #data_ident: [u8; (#sizes) / 8],
            }

            impl #ident {
                #constructor

                #accessors
            }
        }
    }

    fn expand_constructor(&self, field: &Ident, size: &Punctuated<ExprPath, Token![+]>) -> proc_macro2::TokenStream {
        quote! {
            fn new() -> Self {
                Self {
                    #field: [0; (#size) / 8],
                }
            }
        }
    }

    fn expand_accessors(&self, fields: &Vec<(Ident, Type)>) -> proc_macro2::TokenStream {
        quote! {}
    }
}

impl syn::parse::Parse for Bitfield {
    fn parse(input: &ParseBuffer) -> Result<Self, Error> {
        Ok(Self {
            ast: input.parse()?,
        })
    }
}
