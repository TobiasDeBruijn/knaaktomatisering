use proc_macro::TokenStream;
use syn::DeriveInput;

mod string_like;

/// Derive macro to make a type of form `struct Foo(String)` behave a little
/// more like a String.
///
/// Adds:
/// - [std::fmt::Display]
/// - [AsRef] `<str>`
#[proc_macro_derive(StringLike)]
pub fn string_like(ts: TokenStream) -> TokenStream {
    let ts: DeriveInput = match syn::parse(ts) {
        Ok(ts) => ts,
        Err(e) => return e.to_compile_error().into(),
    };

    string_like::apply(ts)
}
