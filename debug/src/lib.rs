use proc_macro::TokenStream;
use syn::{self, spanned::Spanned, parse_quote};
use quote::{ToTokens, quote};
use std::io::{self, Write};

// Must have `attributes(debug)`, or
// ┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈
// error: cannot find attribute `debug` in this scope
//   --> tests/03-custom-format.rs:29:7
//    |
// 29 |     #[debug = "0b{:08b}"]
//    |       ^^^^^
// ┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈┈

#[proc_macro_derive(CustomDebug, attributes(debug))]
pub fn derive(input: TokenStream) -> TokenStream {
    let st = syn::parse_macro_input!(input as syn::DeriveInput);
    match do_expand(&st) {
        Ok(token_stream) => token_stream.into(),
        Err(e) => {
            e.to_compile_error().into()
        },
    }
}

fn do_expand(st: &syn::DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let ret = generate_debug_trait(st)?;
    return Ok(ret);
}

type StructFields = syn::punctuated::Punctuated<syn::Field,syn::Token!(,)>;
fn get_fields_from_derive_input(d: &syn::DeriveInput) -> syn::Result<&StructFields> {
    eprint!("!!!! get_fields_from_derive_input {:#?}", d);
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

fn generate_debug_trait_core(st: &syn::DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let fields = get_fields_from_derive_input(st)?;
    eprint!("!!! fields_core[{}] {:#?}", fields.len(), fields);
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
    eprint!("!!! Finish fields");
    Ok(fmt_body_stream)
}

fn get_field_type_name(field: &syn::Field) -> syn::Result<Option<String>> {
    if let syn::Type::Path(syn::TypePath{path: syn::Path{ref segments, ..}, ..}) = field.ty {
        if let Some(syn::PathSegment{ref ident,..}) = segments.last() {
            eprint!("!!! ident: {:?}", ident);
            return Ok(Some(ident.to_string()))
        }
    }
    return Ok(None)
}

fn generate_debug_trait(st: &syn::DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let fmt_body_stream = generate_debug_trait_core(st)?;
    let struct_name_ident = &st.ident;

    let mut generics_param_to_modify = st.generics.clone();

    let fields = get_fields_from_derive_input(st)?;
    let mut field_type_names = Vec::new();
    let mut phantomdata_type_param_names = Vec::new();
    eprint!("!!!! fields {} {:?}", fields.len(), fields);
    for field in fields {
        if let Some(s) = get_field_type_name(field)? {
            eprint!("!!!! field_type_names.push {:?}", s);
            field_type_names.push(s);
        }
        if let Some(s) = get_phantomdata_generic_type_name(field)? {
            eprint!("!!!! phantomdata_type_param_names.push {:?}", s);
            phantomdata_type_param_names.push(s);
        }
    }

    for mut g in generics_param_to_modify.params.iter_mut() {
        if let syn::GenericParam::Type(t) = g {
            let type_param_name = t.ident.to_string();
            // #[derive(CustomDebug)]
            // pub struct Field<T> {
            //     marker: PhantomData<T>,
            //     string: S,
            //     #[debug = "0b{:08b}"]
            //     bitmask: u8,
            // }
            // phantomdata_type_param_names [] field_type_names ["T", "u8"] type_param_name "T"
            if phantomdata_type_param_names.contains(&type_param_name) && !field_type_names.contains(&type_param_name) {
                eprint!("!!! CONT phantomdata_type_param_names {:?} field_type_names {:?} type_param_name {:?}", phantomdata_type_param_names, field_type_names, type_param_name);
                continue;
            } else {
                eprint!("!!! NOCONT phantomdata_type_param_names {:?} field_type_names {:?} type_param_name {:?}", phantomdata_type_param_names, field_type_names, type_param_name);
            }
            t.bounds.push(parse_quote!(std::fmt::Debug));
        }
    }

    // for mut g in generics_param_to_modify.params.iter_mut() {
    //     if let syn::GenericParam::Type(t) = g {
    //         let q = parse_quote!(std::fmt::Debug);
    //         // t: Trait(TraitBound { paren_token: None, modifier: None, lifetimes: None, path: Path { leading_colon: None, segments: [PathSegment { ident: Ident { ident: "std", span: #5 bytes(862..873) }, arguments: None }, Colon2, PathSegment { ident: Ident { ident: "fmt", span: #5 bytes(862..873) }, arguments: None }, Colon2, PathSegment { ident: Ident { ident: "Debug", span: #5 bytes(862..873) }, arguments: None }] } })
    //         t.bounds.push(q);
    //     }
    // }

    eprint!("!!! generics_param_to_modify {:?}", generics_param_to_modify);
    let (impl_generics, type_generics, where_clause) = generics_param_to_modify.split_for_impl();

    eprint!("!!! impl_generics {:?}", impl_generics);

    // impl<T: std::fmt::Debug> std::fmt::Debug for Field<T> {
    let ret_stream = quote!(
        impl #impl_generics std::fmt::Debug for #struct_name_ident #type_generics #where_clause {
            fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
                #fmt_body_stream
            }
        }
    );

    // eprint!("!!! return {:#?}", ret_stream);
    Ok(ret_stream)
}

fn get_phantomdata_generic_type_name(field: &syn::Field) -> syn::Result<Option<String>> {
    if let syn::Type::Path(syn::TypePath{path: syn::Path{ref segments, ..}, ..}) = field.ty {
        // eprint!("!!! get_phantomdata_generic_type_name 111 {:?}", field);
        if let Some(syn::PathSegment{ref ident, ref arguments}) = segments.last() {
            // eprint!("!!! get_phantomdata_generic_type_name 222");
            if ident == "PhantomData" {
                // eprint!("!!! get_phantomdata_generic_type_name 333");
                if let syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments{args, ..}) = arguments {
                    // eprint!("!!! get_phantomdata_generic_type_name 444");
                    if let Some(syn::GenericArgument::Type(syn::Type::Path( ref gp))) = args.first() {
                        // eprint!("!!! get_phantomdata_generic_type_name 555");
                        if let Some(generic_ident) = gp.path.segments.first() {
                            // eprint!("!!! get_phantomdata_generic_type_name 666");
                            return Ok(Some(generic_ident.ident.to_string()))
                        }
                    }
                }
            }
        }
    }
    return Ok(None)
}