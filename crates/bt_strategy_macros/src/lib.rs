use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{parse_macro_input, ItemFn, LitStr, Result, Token};

struct StrategyArgs {
    id: LitStr,
    display_name: LitStr,
    description: LitStr,
}

impl Parse for StrategyArgs {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut id: Option<LitStr> = None;
        let mut display_name: Option<LitStr> = None;
        let mut description: Option<LitStr> = None;

        while !input.is_empty() {
            let ident: syn::Ident = input.parse()?;
            input.parse::<Token![=]>()?;

            match ident.to_string().as_str() {
                "id" => id = Some(input.parse()?),
                "display_name" => display_name = Some(input.parse()?),
                "description" => description = Some(input.parse()?),
                _ => return Err(syn::Error::new(ident.span(), "unknown bt_strategy argument")),
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(StrategyArgs {
            id: id.ok_or_else(|| syn::Error::new(proc_macro2::Span::call_site(), "missing id"))?,
            display_name: display_name
                .ok_or_else(|| syn::Error::new(proc_macro2::Span::call_site(), "missing display_name"))?,
            description: description
                .ok_or_else(|| syn::Error::new(proc_macro2::Span::call_site(), "missing description"))?,
        })
    }
}

#[proc_macro_attribute]
pub fn bt_strategy(args: TokenStream, input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as StrategyArgs);
    let item = parse_macro_input!(input as ItemFn);

    let id = args.id;
    let display_name = args.display_name;
    let description = args.description;
    let vis = &item.vis;
    let sig = &item.sig;
    let block = &item.block;

    let expanded = quote! {
        #vis #sig #block

        bt_strategy_sdk::inventory::submit! {
            bt_strategy_sdk::StrategyRegistration {
                metadata: bt_strategy_sdk::StrategyMetadata {
                    id: #id,
                    display_name: #display_name,
                    description: #description,
                    parameters: &[],
                }
            }
        }
    };

    expanded.into()
}
