use proc_macro::{self, TokenStream};
use quote::quote;
use syn::{parse_macro_input,  ItemImpl, ImplItem};
use syn::__private::TokenStream2;

// Impelment a trait Module<'static str, [(&'static str, function_ptr)]> for a type
// basicially turning a impl into a useable array of function pointers
#[proc_macro_attribute]
pub fn module(name: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemImpl);

    let methods: Vec<_> = input
        .items.iter()
        .filter_map(|item| match item {
            ImplItem::Fn(method) => { Some(method) },
            _ => None
        }).collect();

    let identifiers: Vec<_> = methods.iter()
        .map(|f| {
            f.sig.ident.clone()
        }).collect();

    let lib_name = input.self_ty.clone();

    let list: TokenStream2 = identifiers.iter()
        .map(|n| {
            let name = n.to_string();
            quote! {
                (#name, (#lib_name::#n) as *const u8),
            }
        }).collect();

    let len = identifiers.len();

    let name = name.to_string();
    let output = quote! {
        #input

        impl Module<&'static str, [(&'static str, *const u8); #len]> for #lib_name {
            const NAME: &'static str = #name;

            fn symbols() -> [(&'static str, *const u8); #len] {
                [#list]
            }
        }
    };
    output.into()
}
