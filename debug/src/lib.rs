use proc_macro::TokenStream;
use syn::{self, spanned::Spanned};
use quote::{ToTokens, quote};
use std::io::{self, Write};

#[proc_macro_derive(CustomDebug)]
pub fn derive(input: TokenStream) -> TokenStream {
    let st = syn::parse_macro_input!(input as syn::DeriveInput);
    match do_expand(&st) {
        Ok(token_stream) => token_stream.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn do_expand(st: &syn::DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let ret = generate_debug_trait(st)?;
    return Ok(ret);
}

type StructFields = syn::punctuated::Punctuated<syn::Field,syn::Token!(,)>;
fn get_fields_from_derive_input(d: &syn::DeriveInput) -> syn::Result<&StructFields> {
    if let syn::Data::Struct(syn::DataStruct {
        fields: syn::Fields::Named(syn::FieldsNamed { ref named, .. }),
        ..
    }) = d.data{
        return Ok(named)
    }
    Err(syn::Error::new_spanned(d, "Must define on a Struct, not Enum".to_string()))
}

fn generate_attr(field: &syn::Field) -> syn::Result<Option<String>> {
    for attr in field.attrs.iter() {
        if let Ok(syn::Meta::NameValue(nv)) = attr.parse_meta() {
            eprint!("!!! nv {:#?}", nv);
            io::stderr().flush().unwrap();
            if let Some(id) = nv.path.get_ident() {
                if id == "debug" {
                    if let syn::Lit::Str(ref ident_str) = nv.lit {
                        return Ok(Some(
                            ident_str.value()
                        ));
                    }
                }
            }
        }
    }
    Ok(None)
}

fn generate_debug_trait(st: &syn::DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let fields = get_fields_from_derive_input(st)?;
    eprint!("!!! fields {:#?}", fields);
    io::stderr().flush().unwrap();
    let struct_name_ident = &st.ident;
    let struct_name_literal = struct_name_ident.to_string();

    let mut fmt_body_stream = proc_macro2::TokenStream::new();

    fmt_body_stream.extend(quote!(
        fmt.debug_struct(#struct_name_literal) // 注意这里引用的是一个字符串，不是一个syn::Ident，生成的代码会以字面量形式表示出来
    ));
    for field in fields.iter(){
        let field_name_idnet = field.ident.as_ref().unwrap();
        let field_name_literal = field_name_idnet.to_string();
        if let Ok(Some(debug)) = generate_attr(field) {
            eprint!("!!! Found");
            fmt_body_stream.extend(quote!(
                .field(#field_name_literal, &format_args!(#debug, self.#field_name_idnet))
            ));
        } else {
            eprint!("!!! NoOK");
            fmt_body_stream.extend(quote!(
                .field(#field_name_literal, &self.#field_name_idnet)
            ));
        }
    }

    fmt_body_stream.extend(quote!(
        .finish()
    ));

    let ret_stream = quote!(
        impl std::fmt::Debug for #struct_name_ident {
            fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
                #fmt_body_stream
            }
        }
    );

    return Ok(ret_stream)
}