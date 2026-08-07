use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Error, Fields, parse_macro_input};

/// Derives [`ModuleVoice`] for a struct that has a `triggered: Option<usize>` field.
///
/// ```ignore
/// #[derive(ModuleVoice)]
/// struct Voice {
///     triggered: Option<usize>,
///     // ...
/// }
/// ```
#[proc_macro_derive(ModuleVoice)]
pub fn derive_module_voice(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match expand_module_voice(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn expand_module_voice(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let Data::Struct(data_struct) = &input.data else {
        return Err(Error::new_spanned(
            name,
            "ModuleVoice can only be derived for structs",
        ));
    };

    let Fields::Named(fields) = &data_struct.fields else {
        return Err(Error::new_spanned(
            name,
            "ModuleVoice requires a struct with named fields including `triggered: Option<usize>`",
        ));
    };

    let has_triggered = fields.named.iter().any(|field| {
        field
            .ident
            .as_ref()
            .is_some_and(|ident| ident == "triggered")
    });

    if !has_triggered {
        return Err(Error::new_spanned(
            name,
            "ModuleVoice requires a `triggered` field",
        ));
    }

    Ok(quote! {
        impl #impl_generics crate::synth_engine::synth_module::ModuleVoice
            for #name #ty_generics #where_clause
        {
            #[inline]
            fn triggered(&self) -> Option<usize> {
                self.triggered
            }

            #[inline]
            fn clear_triggered(&mut self) {
                self.triggered = None;
            }
        }
    })
}
