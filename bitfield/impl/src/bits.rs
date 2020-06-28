use syn::Token;

pub(crate) struct GenerateBits {
    pub(crate) from: usize,
    pub(crate) to: usize,
}

impl syn::parse::Parse for GenerateBits {
    fn parse(input: syn::parse::ParseStream) -> syn::parse::Result<Self> {
        let from_literal: syn::LitInt = input.parse()?;
        let inclusive = input.peek(Token![..=]);
        if inclusive {
            input.parse::<Token![..=]>()?;
        } else {
            input.parse::<Token![..]>()?;
        }
        let to_literal: syn::LitInt = input.parse()?;

        let from = from_literal.base10_parse::<usize>()?;
        let to = if inclusive {
            to_literal.base10_parse::<usize>()? + 1
        } else {
            to_literal.base10_parse::<usize>()?
        };

        Ok(Self {
            from,
            to,
        })
    }
}
