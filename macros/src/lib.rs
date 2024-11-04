use proc_macro::TokenStream as TokenStream1;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Error, ItemFn, ReturnType, Signature, Type};

/// The `#[main]` attribute macro, which is used to mark the entry point of the program.
///
/// This macro must be attached to a function that takes no arguments, and never returns: () -> !.
#[proc_macro_attribute]
pub fn main(args: TokenStream1, input: TokenStream1) -> TokenStream1 {
    if !args.is_empty() {
        return Error::new_spanned(TokenStream::from(args), "this macro takes no arguments")
            .into_compile_error()
            .into();
    }

    match modify_fn(parse_macro_input!(input as ItemFn)) {
        Ok(stream) => stream.into(),
        Err(err) => err.into_compile_error().into(),
    }
}

fn modify_fn(
    ItemFn {
        attrs,
        vis,
        sig:
            Signature {
                constness,
                asyncness,
                unsafety,
                abi,
                fn_token,
                ident,
                generics,
                inputs,
                variadic,
                output,
                ..
            },
        block,
    }: ItemFn,
) -> Result<TokenStream, Error> {
    if asyncness.is_some() {
        return Err(Error::new_spanned(
            asyncness,
            "main function cannot be async",
        ));
    }
    if unsafety.is_some() {
        return Err(Error::new_spanned(
            unsafety,
            "main function cannot be unsafe",
        ));
    }
    if abi.is_some() {
        return Err(Error::new_spanned(
            abi,
            "main function must use the default ABI",
        ));
    }
    if !generics.params.is_empty() {
        return Err(Error::new_spanned(
            generics,
            "main function cannot have generic parameters",
        ));
    }
    if !inputs.is_empty() {
        return Err(Error::new_spanned(
            inputs,
            "main function cannot have parameters",
        ));
    }
    if variadic.is_some() {
        return Err(Error::new_spanned(
            variadic,
            "main function cannot be variadic",
        ));
    }
    let ReturnType::Type(rt_token, return_type) = output else {
        return Err(Error::new_spanned(
            output,
            "main function must never return (`-> !`)",
        ));
    };
    let &Type::Never(_) = return_type.as_ref() else {
        return Err(Error::new_spanned(
            return_type,
            "main function return type must be `!`",
        ));
    };
    let output = ReturnType::Type(rt_token, return_type);
    Ok(quote! {
        #(#attrs)*
        #[unsafe(export_name = "_main")]
        #vis #constness #fn_token #ident () #output {
            #block
        }
    })
}
