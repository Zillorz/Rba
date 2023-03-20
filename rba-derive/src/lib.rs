use proc_macro::{self, TokenStream};
use quote::quote;
use syn::{parse_macro_input,  ItemImpl, ImplItem};
use syn::__private::TokenStream2;

#[proc_macro_attribute]
pub fn module(name: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemImpl);

    let methods: Vec<_> = input
        .items.iter()
        .filter_map(|item| match item {
            ImplItem::Fn(method) => { Some(method) },
            _ => None
        }).collect();

    let idents: Vec<_> = methods.iter()
        .map(|f| {
            f.sig.ident.clone()
        }).collect();

    let t = input.self_ty.clone();

    let insert: TokenStream2 = idents.iter()
        .map(|n| {
            let name = n.to_string();
            quote! {
                (#name, (#t::#n) as *const u8),
            }
        }).collect();
    let l = idents.len();

    let name = name.to_string();
    let output = quote! {
        #input

        impl Module<&'static str, [(&'static str, *const u8); #l]> for #t {
            const NAME: &'static str = #name;

            fn symbols() -> [(&'static str, *const u8); #l] {
                [#insert]
            }
        }
    };
    output.into()
}