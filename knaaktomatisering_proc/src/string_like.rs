use proc_macro::TokenStream;
use syn::__private::quote::quote;
use syn::spanned::Spanned;
use syn::{Data, DeriveInput, Error, Fields};

pub fn apply(input: DeriveInput) -> TokenStream {
    let ident = &input.ident;
    match &input.data {
        Data::Struct(struct_data) => match &struct_data.fields {
            Fields::Unnamed(f) => {
                if f.unnamed.len() > 1 {
                    return Error::new(
                        input.span(),
                        "Macro can only be applied to structs with 1 unnamed field",
                    )
                    .to_compile_error()
                    .into();
                }

                quote! {
                    impl ::std::fmt::Display for #ident {
                        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> std::fmt::Result {
                            write!(f, "{}", self.0.to_string())
                        }
                    }

                    impl ::std::convert::AsRef<str> for #ident {
                        fn as_ref(&self) -> &str {
                            &self.0
                        }
                    }
                }
            }
            Fields::Unit => Error::new(input.span(), "Macro cannot be applied to Unit fields")
                .to_compile_error(),
            Fields::Named(_) => Error::new(
                input.span(),
                "Macro cannot be applied to a struct with named fields",
            )
            .to_compile_error(),
        },
        _ => Error::new(
            input.span(),
            "Macro can only be applied to structs with 1 unnamed field",
        )
        .to_compile_error(),
    }
    .into()
}
