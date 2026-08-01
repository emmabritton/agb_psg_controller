
use proc_macro::TokenStream;
use quote::quote;
use std::fs;
use std::path::Path;
use syn::LitStr;

#[proc_macro]
pub fn include_pmus(input: TokenStream) -> TokenStream {
    include_common(input, Kind::Song)
}

#[proc_macro]
pub fn include_psfx(input: TokenStream) -> TokenStream {
    include_common(input, Kind::Sfx)
}

enum Kind {
    Song,
    Sfx,
}

impl Kind {
    fn extension(&self) -> &'static str {
        match self {
            Kind::Song => "pmus",
            Kind::Sfx => "psfx",
        }
    }

    fn macro_name(&self) -> &'static str {
        match self {
            Kind::Song => "include_pmus!",
            Kind::Sfx => "include_psfx!",
        }
    }
}

fn include_common(input: TokenStream, kind: Kind) -> TokenStream {
    let lit = match syn::parse::<LitStr>(input) {
        Ok(lit) => lit,
        Err(e) => return e.to_compile_error().into(),
    };
    let filename = lit.value();

    let expected = kind.extension();
    if !Path::new(&filename)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case(expected))
    {
        return syn::Error::new(
            lit.span(),
            format!(
                "{} expects a `.{expected}` file, got \"{filename}\" \
                 (songs are `.pmus`, sound effects are `.psfx`)",
                kind.macro_name()
            ),
        )
        .to_compile_error()
        .into();
    }

    let root = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set");
    let path = Path::new(&root).join(&filename);
    let include_path = path.to_string_lossy().into_owned();

    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) => {
            return syn::Error::new(lit.span(), format!("cannot read {}: {e}", path.display()))
                .to_compile_error()
                .into();
        }
    };

    let parsed = match kind {
        Kind::Song => eb_agb_psg_interop::parse::parse_song(&text).map(|track| quote!(#track)),
        Kind::Sfx => eb_agb_psg_interop::parse::parse_sfx(&text).map(|sfx| quote!(#sfx)),
    };

    match parsed {
        Ok(value) => quote! {{
            const _: &[u8] = include_bytes!(#include_path);
            #value
        }}
        .into(),
        Err(e) => syn::Error::new(lit.span(), format!("{filename}: {}", e.message))
            .to_compile_error()
            .into(),
    }
}
