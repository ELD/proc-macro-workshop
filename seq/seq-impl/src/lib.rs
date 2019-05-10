extern crate proc_macro;

use proc_macro_hack::proc_macro_hack;
use proc_macro::TokenStream;
use syn::{parse_macro_input, Token};

#[derive(Debug, Clone, Copy)]
enum Mode {
    ReplaceIdent(usize),
    ExpandSequence,
}

struct SeqMacroInput {
    ident: syn::Ident,
    from: syn::LitInt,
    to: syn::LitInt,
    inclusive: bool,
    tt: proc_macro2::TokenStream,
}

impl Into<TokenStream> for SeqMacroInput {
    fn into(self) -> TokenStream {
        let mut expanded = false;
        let out = self.expand(self.tt.clone(), &mut expanded, Mode::ExpandSequence);
        if expanded {
            return out.into();
        }

        self.range()
            .map(|i| self.expand(self.tt.clone(), &mut expanded, Mode::ReplaceIdent(i)))
            .collect::<proc_macro2::TokenStream>()
            .into()
    }
}

impl SeqMacroInput {
    fn range(&self) -> std::ops::Range<usize> {
        if self.inclusive {
            (self.from.base10_parse::<usize>().unwrap()..self.to.base10_parse::<usize>().unwrap() + 1)
        } else {
            (self.from.base10_parse::<usize>().unwrap()..self.to.base10_parse::<usize>().unwrap())
        }
    }

    fn expand(&self, tt: proc_macro2::TokenStream, expanded: &mut bool, mode: Mode) -> proc_macro2::TokenStream {
        let mut out = proc_macro2::TokenStream::new();
        let mut tts = tt.into_iter();
        while let Some(tt) = tts.next() {
            out.extend(self.expand2(tt, &mut tts, expanded, mode));
        }

        out
    }

    fn expand2(
        &self,
        tt: proc_macro2::TokenTree,
        rest: &mut proc_macro2::token_stream::IntoIter,
        expanded: &mut bool,
        mode: Mode,
    ) -> proc_macro2::TokenStream {
        let tt = match tt {
            proc_macro2::TokenTree::Group(g) => {
                let mut expanded = proc_macro2::Group::new(g.delimiter(), self.expand(g.stream(), expanded, mode));
                expanded.set_span(g.span());
                proc_macro2::TokenTree::Group(expanded)
            }
            proc_macro2::TokenTree::Ident(ref ident) if ident == &self.ident => {
                if let Mode::ReplaceIdent(i) = mode {
                    let mut lit = proc_macro2::Literal::usize_unsuffixed(i);
                    lit.set_span(ident.span());
                    *expanded = true;
                    proc_macro2::TokenTree::Literal(lit)
                } else {
                    proc_macro2::TokenTree::Ident(ident.clone())
                }
            }
            proc_macro2::TokenTree::Ident(mut ident) => {
                let mut peek = rest.clone();
                match (mode, peek.next(), peek.next()) {
                    (
                        Mode::ReplaceIdent(i),
                        Some(proc_macro2::TokenTree::Punct(ref punct)),
                        Some(proc_macro2::TokenTree::Ident(ref ident2))
                    ) if punct.as_char() == '#' && ident2 == &self.ident => {
                        ident = proc_macro2::Ident::new(&format!("{}{}", ident, i), ident.span());
                        *rest = peek.clone();
                        *expanded = true;

                        match peek.next() {
                            Some(proc_macro2::TokenTree::Punct(ref punct))
                            if punct.as_char() == '#' =>
                                {
                                    *rest = peek.clone();
                                }
                            _ => {}
                        }
                    }
                    _ => {}
                }

                proc_macro2::TokenTree::Ident(ident)
            }
            proc_macro2::TokenTree::Punct(ref p) if p.as_char() == '#' => {
                let mut peek = rest.clone();
                match (peek.next(), peek.next()) {
                    (
                        Some(proc_macro2::TokenTree::Group(ref rep)),
                        Some(proc_macro2::TokenTree::Punct(ref star))
                    ) if rep.delimiter() == proc_macro2::Delimiter::Parenthesis
                        && star.as_char() == '*' =>
                        {
                            *expanded = true;
                            *rest = peek;
                            return self.range()
                                .map(|i| self.expand(rep.stream(), expanded, Mode::ReplaceIdent(i)))
                                .collect::<proc_macro2::TokenStream>();
                        }
                    _ => {}
                }
                proc_macro2::TokenTree::Punct(p.clone())
            }
            tt => tt,
        };

        std::iter::once(tt).collect()
    }
}

impl syn::parse::Parse for SeqMacroInput {
    fn parse(input: syn::parse::ParseStream) -> syn::parse::Result<Self> {
        let ident: syn::Ident = input.parse()?;
        input.parse::<Token![in]>()?;
        let from: syn::LitInt = input.parse()?;
        let inclusive = input.peek(Token![..=]);
        if inclusive {
            input.parse::<Token![..=]>()?;
        } else {
            input.parse::<Token![..]>()?;
        }
        let to: syn::LitInt = input.parse()?;
        let content;
        syn::braced!(content in input);
        let tt: proc_macro2::TokenStream = content.parse()?;

        Ok(Self {
            ident,
            from,
            to,
            inclusive,
            tt,
        })
    }
}

#[proc_macro]
pub fn seq(input: TokenStream) -> TokenStream {
    let seq: SeqMacroInput = parse_macro_input!(input as SeqMacroInput);

    seq.into()
}

#[proc_macro_hack]
pub fn eseq(input: TokenStream) -> TokenStream {
    let tokens = seq(input);
    tokens
}
