extern crate proc_macro;

use proc_macro::TokenStream;

use syn::visit_mut::VisitMut;
use syn::export::ToTokens;
use syn::spanned::Spanned;

struct LexicographicSorting {
    sorting_errors: Vec<syn::Error>,
}

impl Default for LexicographicSorting {
    fn default() -> Self {
        Self {
            sorting_errors: Vec::new(),
        }
    }
}

impl VisitMut for LexicographicSorting {
    fn visit_expr_match_mut(&mut self, i: &mut syn::ExprMatch) {
        if !i.attrs.iter().any(|attr| attr.path.is_ident("sorted")) { return; }

        let mut wildcard: Option<syn::PatWild> = None;
        let mut arms = Vec::new();
        for arm in i.arms.iter() {
            if let Some(w) = wildcard {
                self.sorting_errors.push(syn::Error::new(
                    w.span(),
                    "wildcard should be listed last",
                ));
                break;
            }

            let path = if let Some(path) = path_from_arm_pattern(&arm.pat) {
                path
            } else if let syn::Pat::Wild(ref w) = arm.pat {
                wildcard = Some(w.clone());
                continue;
            } else {
                self.sorting_errors.push(syn::Error::new_spanned(
                    &arm.pat,
                    "unsupported by #[sorted]",
                ));
                continue;
            };

            let path_string = path_as_string(&path);
            if arms.last().map(|last| &path_string < last).unwrap_or(false) {
                let next_lexicographic_arm_idx = arms.binary_search(&path_string).unwrap_or(0);
                self.sorting_errors.push(syn::Error::new_spanned(
                    path,
                    format!("{} should sort before {}", path_string, arms[next_lexicographic_arm_idx]),
                ));
            }
            arms.push(path_string);
        }

        i.attrs.clear();
    }
}

fn parse_sorted(item: syn::Item) -> Result<(), syn::Error> {
    match item {
        syn::Item::Enum(e) => {
            let mut names: Vec<String> = Vec::new();
            for variant in e.variants.iter() {
                let variant_name = variant.ident.to_string();
                if names.last().map(|last| &variant_name < last).unwrap_or(false) {
                    let next_lexicographic_variant_idx = names.binary_search(&variant_name).unwrap_or(0);
                    return Err(syn::Error::new(
                        variant.ident.span(),
                        format!("{} should sort before {}", variant_name, names[next_lexicographic_variant_idx]),
                    ));
                }
                names.push(variant_name);
            }

            Ok(())
        }
        _ => Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "expected enum or match expression",
        ))
    }
}

fn path_from_arm_pattern(pattern: &syn::Pat) -> Option<syn::Path> {
    match *pattern {
        syn::Pat::TupleStruct(ref ts) => Some(ts.path.clone()),
        syn::Pat::Ident(syn::PatIdent { ident: ref i, .. }) => Some(i.clone().into()),
        syn::Pat::Struct(ref s) => Some(s.path.clone()),
        syn::Pat::Path(ref p) => Some(p.path.clone()),
        _ => None,
    }
}

fn path_as_string(path: &syn::Path) -> String {
    path.segments.iter()
        .map(|seg| seg.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

#[proc_macro_attribute]
pub fn sorted(args: TokenStream, input: TokenStream) -> TokenStream {
    let mut out = input.clone();
    assert!(args.is_empty());

    let item = syn::parse_macro_input!(input as syn::Item);

    if let Err(e) = parse_sorted(item) {
        out.extend(TokenStream::from(e.to_compile_error()));
    }

    out
}

#[proc_macro_attribute]
pub fn check(args: TokenStream, input: TokenStream) -> TokenStream {
    assert!(args.is_empty());

    let mut item_fn = syn::parse_macro_input!(input as syn::ItemFn);
    let mut sorter = LexicographicSorting::default();

    sorter.visit_item_fn_mut(&mut item_fn);
    let mut out = item_fn.into_token_stream();

    sorter.sorting_errors.iter()
        .take(1)
        .for_each(|e| out.extend(e.to_compile_error()));

    out.into()
}
