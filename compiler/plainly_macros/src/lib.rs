//! The `procedure` attribute for `extern.rust` procedures (D108), used as `#[plainly::procedure("name")]` through the
//! `plainly` crate.
//!
//! It names the Plainly procedure a Rust function carries out, checks the function's parameters while building, and
//! writes the glue Cranelift's code calls: a function with a fixed symbol taking the raw values, turning each `Text`
//! into a handle (D105), passing `Whole` and `Bool` straight through (D109), and giving the result back. The symbol
//! spells out the procedure's shape, as in `plainly_procedure_shout_takes_text_gives_text`, so a Rust function and a
//! Plainly declaration that disagree can't be linked together.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::visit_mut::VisitMut;
use syn::{
    FnArg, GenericArgument, GenericParam, Ident, ItemFn, Lifetime, LifetimeParam, LitStr, PathArguments, ReturnType,
    Token, Type, parse_macro_input,
};

/// The attribute's words: the Plainly procedure's name, then `fails = "..."` for one that can fail (D110).
struct Attribute {
    name: LitStr,
    fails: Option<LitStr>,
}

impl Parse for Attribute {
    fn parse(input: ParseStream) -> syn::Result<Attribute> {
        let name = input.parse()?;
        let mut fails = None;
        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            let key: Ident = input.parse()?;
            if key != "fails" {
                return Err(syn::Error::new_spanned(key, "the only setting after the name is `fails = \"...\"`"));
            }
            input.parse::<Token![=]>()?;
            fails = Some(input.parse()?);
        }
        Ok(Attribute { name, fails })
    }
}

/// A failure's name in Rust's casing: `not_found` as `NotFound`.
fn rust_casing(name: &str) -> String {
    name.split('_')
        .map(|word| {
            let mut letters = word.chars();
            letters
                .next()
                .map(|first| first.to_ascii_uppercase().to_string() + letters.as_str())
                .unwrap_or_default()
        })
        .collect()
}

/// Turns every bare `Failure` in the signature into the procedure's own, in its module.
struct FailurePath {
    module: Ident,
}

impl VisitMut for FailurePath {
    fn visit_type_path_mut(&mut self, path: &mut syn::TypePath) {
        if path.qself.is_none() && path.path.segments.len() == 1 && path.path.segments[0].ident == "Failure" {
            let module = &self.module;
            *path = syn::parse_quote!(#module::Failure);
        }
        syn::visit_mut::visit_type_path_mut(self, path);
    }
}

/// The `T` and the failure type of `Result<T, Failure>`.
fn result_parts(ty: &Type) -> Option<(Type, Type)> {
    let Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    if segment.ident != "Result" {
        return None;
    }
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    let mut types = arguments.args.iter().filter_map(|argument| match argument {
        GenericArgument::Type(inner) => Some(inner.clone()),
        _ => None,
    });
    Some((types.next()?, types.next()?))
}

/// The values that cross into Rust (D105, D109).
#[derive(Clone, Copy)]
enum Kind {
    Text,
    Whole,
    Bool,
}

impl Kind {
    fn word(self) -> &'static str {
        match self {
            Kind::Text => "text",
            Kind::Whole => "whole",
            Kind::Bool => "bool",
        }
    }
}

/// The last name in a type's path, like `Handle` in `plainly::Handle<Text>`.
fn last_name(ty: &Type) -> Option<String> {
    match ty {
        Type::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        _ => None,
    }
}

fn kind_of(ty: &Type) -> Option<Kind> {
    let Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    match segment.ident.to_string().as_str() {
        "i64" => Some(Kind::Whole),
        "bool" => Some(Kind::Bool),
        "Handle" => {
            let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
                return None;
            };
            arguments
                .args
                .iter()
                .any(|argument| matches!(argument, GenericArgument::Type(inner) if last_name(inner).as_deref() == Some("Text")))
                .then_some(Kind::Text)
        }
        _ => None,
    }
}

fn is_call(ty: &Type) -> bool {
    matches!(ty, Type::Reference(reference) if reference.mutability.is_some() && last_name(&reference.elem).as_deref() == Some("Call"))
}

/// Gives every `Call` and `Handle` written without a lifetime the call's, so `fn shout(call: &mut Call, text:
/// Handle<Text>) -> Handle<Text>` means what it says.
struct CallLifetime {
    lifetime: Lifetime,
    added: bool,
}

impl VisitMut for CallLifetime {
    fn visit_path_segment_mut(&mut self, segment: &mut syn::PathSegment) {
        let name = segment.ident.to_string();
        if name == "Call" || name == "Handle" {
            let lifetime = GenericArgument::Lifetime(self.lifetime.clone());
            match &mut segment.arguments {
                PathArguments::None => {
                    segment.arguments =
                        PathArguments::AngleBracketed(syn::parse_quote!(<#lifetime>));
                    self.added = true;
                }
                PathArguments::AngleBracketed(arguments)
                    if !arguments
                        .args
                        .iter()
                        .any(|argument| matches!(argument, GenericArgument::Lifetime(_))) =>
                {
                    arguments.args.insert(0, lifetime);
                    self.added = true;
                }
                _ => {}
            }
        }
        syn::visit_mut::visit_path_segment_mut(self, segment);
    }
}

/// Whether `name` is a Plainly procedure's name: lowercase words with digits, joined by `_`, with narrowings after
/// `.` (D41, D99).
fn is_procedure_name(name: &str) -> bool {
    !name.is_empty()
        && name.split('.').all(|part| {
            part.starts_with(|first: char| first.is_ascii_lowercase())
                && !part.ends_with('_')
                && !part.contains("__")
                && part.chars().all(|letter| {
                    letter.is_ascii_lowercase() || letter.is_ascii_digit() || letter == '_'
                })
        })
}

/// Marks a Rust function as the body of the `extern.rust` procedure named in the attribute (D107, D108).
#[proc_macro_attribute]
pub fn procedure(attribute: TokenStream, item: TokenStream) -> TokenStream {
    let Attribute { name, fails } = parse_macro_input!(attribute as Attribute);
    let mut function = parse_macro_input!(item as ItemFn);
    let plainly_name = name.value();
    let failures: Vec<String> = fails
        .as_ref()
        .map(|list| {
            list.value()
                .split(',')
                .map(|failure| failure.trim().to_string())
                .filter(|failure| !failure.is_empty())
                .collect()
        })
        .unwrap_or_default();
    if let Some(list) = &fails
        && (failures.is_empty() || failures.iter().any(|failure| failure.contains('.') || !is_procedure_name(failure)))
    {
        return syn::Error::new_spanned(list, "these aren't the names of failures, separated by commas")
            .to_compile_error()
            .into();
    }

    let mut inputs = function.sig.inputs.iter();
    let first_is_call = matches!(inputs.next(), Some(FnArg::Typed(first)) if is_call(&first.ty));
    if !first_is_call {
        return syn::Error::new_spanned(
            &function.sig,
            "a Plainly procedure's first parameter is its call: `call: &mut Call`",
        )
        .to_compile_error()
        .into();
    }
    let mut kinds = Vec::new();
    for input in inputs {
        let kind = match input {
            FnArg::Typed(typed) => kind_of(&typed.ty),
            FnArg::Receiver(_) => None,
        };
        let Some(kind) = kind else {
            return syn::Error::new_spanned(
                input,
                "a Plainly procedure's parameter is a `Handle<Text>`, an `i64` or a `bool`",
            )
            .to_compile_error()
            .into();
        };
        kinds.push(kind);
    }
    // With failures, the function gives back `Result<T, Failure>`, where `T` is what the procedure gives (D110).
    let given = match (&function.sig.output, failures.is_empty()) {
        (ReturnType::Default, true) => None,
        (ReturnType::Type(_, ty), true) => Some((**ty).clone()),
        (ReturnType::Type(_, ty), false) => match result_parts(ty) {
            Some((value, failure)) if last_name(&failure).as_deref() == Some("Failure") => match value {
                Type::Tuple(tuple) if tuple.elems.is_empty() => None,
                other => Some(other),
            },
            _ => {
                return syn::Error::new_spanned(ty, "a Plainly procedure that can fail gives back `Result<..., Failure>`")
                    .to_compile_error()
                    .into();
            }
        },
        (ReturnType::Default, false) => {
            return syn::Error::new_spanned(
                &function.sig,
                "a Plainly procedure that can fail gives back `Result<..., Failure>`",
            )
            .to_compile_error()
            .into();
        }
    };
    let gives = match &given {
        None => None,
        Some(ty) => match kind_of(ty) {
            Some(kind) => Some(kind),
            None => {
                return syn::Error::new_spanned(
                    ty,
                    "a Plainly procedure gives back a `Handle<Text>`, an `i64`, a `bool`, or nothing",
                )
                .to_compile_error()
                .into();
            }
        },
    };

    let mut lifetimes = CallLifetime {
        lifetime: Lifetime::new("'plainly_call", proc_macro2::Span::call_site()),
        added: false,
    };
    for input in function.sig.inputs.iter_mut() {
        if let FnArg::Typed(typed) = input {
            lifetimes.visit_type_mut(&mut typed.ty);
        }
    }
    if let ReturnType::Type(_, ty) = &mut function.sig.output {
        lifetimes.visit_type_mut(ty);
    }
    if lifetimes.added {
        function.sig.generics.params.insert(
            0,
            GenericParam::Lifetime(LifetimeParam::new(lifetimes.lifetime.clone())),
        );
    }

    // The procedure's own `Failure`, in a module of its own, so two procedures' failures never clash (D110).
    let module = format_ident!("plainly_failures_{}", function.sig.ident);
    let failure_type = if failures.is_empty() {
        quote!()
    } else {
        let variants: Vec<Ident> = failures.iter().map(|failure| format_ident!("{}", rust_casing(failure))).collect();
        let numbers = (1..=variants.len() as u32).collect::<Vec<_>>();
        FailurePath { module: module.clone() }.visit_return_type_mut(&mut function.sig.output);
        let body = &function.block;
        function.block = syn::parse_quote!({
            #[allow(unused_imports)]
            use #module::Failure;
            #body
        });
        quote! {
            #[doc(hidden)]
            pub mod #module {
                /// The failures this procedure can give back, as its Plainly header lists them.
                #[derive(Clone, Copy, Debug, PartialEq, Eq)]
                pub enum Failure {
                    #(#variants),*
                }

                impl Failure {
                    #[doc(hidden)]
                    pub fn number(self) -> u32 {
                        match self {
                            #(Failure::#variants => #numbers),*
                        }
                    }
                }
            }
        }
    };

    let takes = if kinds.is_empty() {
        "nothing".to_string()
    } else {
        kinds
            .iter()
            .map(|kind| kind.word())
            .collect::<Vec<_>>()
            .join("_")
    };
    let mut symbol_name = format!(
        "plainly_procedure_{}_takes_{}_gives_{}",
        plainly_name.replace('.', "__"),
        takes,
        gives.map_or("nothing", Kind::word)
    );
    if !failures.is_empty() {
        symbol_name.push_str("_fails_");
        symbol_name.push_str(&failures.join("__"));
    }
    let symbol = format_ident!("{}", symbol_name);
    let raw = |kind: Kind| match kind {
        Kind::Text => quote!(*const u8),
        Kind::Whole => quote!(i64),
        Kind::Bool => quote!(bool),
    };
    let arguments: Vec<_> = (0..kinds.len())
        .map(|index| format_ident!("plainly_argument_{index}"))
        .collect();
    let raw_parameters = kinds.iter().zip(&arguments).map(|(kind, argument)| {
        let ty = raw(*kind);
        quote!(#argument: #ty)
    });
    let conversions = kinds
        .iter()
        .zip(&arguments)
        .map(|(kind, argument)| match kind {
            Kind::Text => quote! {
                // SAFETY: Cranelift's code passes a live text's reference, after its load barrier.
                let #argument = unsafe { ::plainly::glue::adopt_text(plainly_call, #argument) };
            },
            Kind::Whole | Kind::Bool => quote!(),
        });
    let rust_name = &function.sig.ident;
    let give_back = |value: proc_macro2::TokenStream| match gives {
        Some(Kind::Text) => quote!(::plainly::glue::give_back_text(plainly_call, #value)),
        _ => value,
    };
    let (result_parameter, raw_result, finish) = if failures.is_empty() {
        let raw_result = gives.map(|kind| {
            let ty = raw(kind);
            quote!(-> #ty)
        });
        (quote!(), raw_result, give_back(quote!(outcome)))
    } else {
        let (result_parameter, write) = match gives {
            Some(kind) => {
                let ty = raw(kind);
                let value = give_back(quote!(value));
                (
                    quote!(plainly_result: *mut #ty,),
                    quote! {
                        // SAFETY: Cranelift's code passes a slot for exactly this value.
                        unsafe { plainly_result.write(#value) };
                    },
                )
            }
            None => (quote!(), quote!(let () = value;)),
        };
        let finish = quote! {
            match outcome {
                Ok(value) => {
                    #write
                    0
                }
                Err(failure) => failure.number(),
            }
        };
        (result_parameter, Some(quote!(-> u32)), finish)
    };
    quote! {
        #failure_type

        #function

        #[doc(hidden)]
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn #symbol(
            #(#raw_parameters,)*
            #result_parameter
            plainly_site: *const u8,
            plainly_site_length: i64
        ) #raw_result {
            ::plainly::glue::with_call(|plainly_call| {
                #(#conversions)*
                // A panic is caught here, at the edge, since unwinding into Cranelift's code would be undefined (D111).
                let outcome = ::plainly::glue::catch(|| #rust_name(plainly_call, #(#arguments),*))
                    .unwrap_or_else(|panic| {
                        ::plainly::glue::rust_went_wrong(#plainly_name, panic, plainly_site, plainly_site_length)
                    });
                #finish
            })
        }
    }
    .into()
}
