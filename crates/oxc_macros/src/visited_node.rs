use proc_macro2::TokenStream as TokenStream2;

use quote::quote;
use syn::{parse_quote, Expr, ExprGroup, ExprLit, Item, ItemEnum, ItemStruct, Lit};

pub fn visited_node(mut item: Item) -> TokenStream2 {
    match &mut item {
        Item::Struct(it) => modify_struct(it),
        Item::Enum(it) => modify_enum(it),
        _ => panic!("`visited_node` attribute can only be used on enums and structs"),
    };

    quote! { #item }
}

fn modify_struct(item: &mut ItemStruct) {
    // Add `#[repr(C)]`
    let mut has_repr_attr = false;
    for attr in &item.attrs {
        if attr.path().is_ident("repr") {
            // TODO: Check is `#[repr(C)]`
            has_repr_attr = true;
        }
    }
    if !has_repr_attr {
        item.attrs.push(parse_quote!(#[repr(C)]));
    }
}

fn modify_enum(item: &mut ItemEnum) {
    // Add `#[repr(C, u8)]`
    let mut has_repr_attr = false;
    for attr in &item.attrs {
        if attr.path().is_ident("repr") {
            // TODO: Check is `#[repr(C, u8)]`
            has_repr_attr = true;
        }
    }
    if !has_repr_attr {
        item.attrs.push(parse_quote!(#[repr(C, u8)]));
    }

    // Add explicit discriminants to all variants
    let mut next_discriminant = 0u8;
    item.variants.iter_mut().for_each(|var| {
        if let Some((.., expr)) = &var.discriminant {
            // Explicit discriminant
            let discriminant = match expr {
                Expr::Lit(ExprLit { lit: Lit::Int(lit), .. }) => {
                    Some(lit.base10_parse::<u8>().unwrap())
                }
                Expr::Group(ExprGroup { expr, .. }) => {
                    if let Expr::Lit(ExprLit { lit: Lit::Int(lit), .. }) = &**expr {
                        Some(lit.base10_parse::<u8>().unwrap())
                    } else {
                        None
                    }
                }
                _ => None,
            };

            if let Some(discriminant) = discriminant {
                next_discriminant = discriminant + 1;
            } else {
                panic!(
                    "`visited_node` attribute only supports integers as explicit discriminators"
                );
            }
        } else {
            // No explicit discriminant - create discriminant following last
            var.discriminant = Some((parse_quote!(=), parse_quote!(#next_discriminant)));
            next_discriminant += 1;
        };
    });
}
