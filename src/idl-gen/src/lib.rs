//! generate rust structs from anchor IDL
use anchor_syn::idl::{
    EnumFields, Idl, IdlEvent, IdlField, IdlType, IdlTypeDefinition, IdlTypeDefinitionTy,
};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::parse_macro_input;

/// generate program event types from given IDL json file
#[proc_macro]
pub fn gen_idl_types(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let path_str = parse_macro_input!(input as syn::LitStr);
    let cargo_manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let path = std::path::PathBuf::from(cargo_manifest_dir).join(path_str.value());
    let idl_json = std::fs::read_to_string(path).expect("file found");
    let idef: Idl = serde_json::from_str(idl_json.as_str()).expect("valid IDL");

    let mut output = TokenStream::new();

    idef.types.iter().for_each(|e| {
        let type_struct = gen_type_struct(e);
        output.extend(vec![type_struct]);
    });

    let mut outer_event_types = TokenStream::new();
    let mut outer_event_impl = TokenStream::new();
    if let Some(events) = idef.events {
        events.iter().for_each(|event| {
            let event_name = syn::Ident::new(event.name.as_str(), Span::call_site());
            // event_names.push(event_name);
            outer_event_types = quote! {
                #outer_event_types
                #event_name(#event_name),
            };
            outer_event_impl = quote! {
                #outer_event_impl
                #event_name::DISCRIMINATOR => Self::#event_name(AnchorDeserialize::deserialize(data).ok()?),
            };

            let event_struct = gen_event_struct(event);
            output = quote! {
                #output
                #event_struct
            };
        });
    }

    let program_event_name = syn::Ident::new(
        format!(
            "{}{}Event",
            (idef.name[..1].to_string()).to_uppercase(),
            &idef.name[1..]
        )
        .as_str(),
        Span::call_site(),
    );
    quote! {
        #output

        #[derive(Debug, PartialEq)]
        pub enum #program_event_name {
            #outer_event_types
        }

        impl #program_event_name {
            fn from_discriminant(disc: [u8; 8], data: &mut &[u8]) -> Option<Self> {
                let event = match disc {
                    #outer_event_impl
                    _ => return None,
                };
                Some(event)
            }
        }
    }
    .into()
}

fn gen_event_struct(event: &IdlEvent) -> TokenStream {
    let event_name = syn::Ident::new(event.name.as_str(), Span::call_site());
    let event_fields: Vec<TokenStream> = event
        .fields
        .iter()
        .map(|f| {
            let f_name = syn::Ident::new(f.name.as_str().trim(), Span::call_site());
            let f_ty: syn::Type =
                syn::parse_str(idl_ty_to_rust_ty(&f.ty).as_str()).expect("valid type");
            quote! {
                pub #f_name: #f_ty,
            }
        })
        .collect();

    quote! {
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[event]
        pub struct #event_name {
            #(#event_fields)*
        }
    }
}

fn gen_type_struct(type_def: &IdlTypeDefinition) -> TokenStream {
    let type_name = syn::Ident::new(type_def.name.as_str(), Span::call_site());

    let res: TokenStream = match type_def.ty {
        IdlTypeDefinitionTy::Enum { ref variants } => {
            let mut variant_ts = TokenStream::new();
            for v in variants {
                let variant_name = syn::Ident::new(v.name.as_str(), Span::call_site());
                match v.fields {
                    Some(EnumFields::Named(ref named)) => {
                        let fields: Vec<TokenStream> =
                            named.iter().map(field_to_token_stream).collect();
                        variant_ts = quote! {
                            #variant_ts

                            #(#fields)*,

                        };
                    }
                    Some(EnumFields::Tuple(ref tuples)) => {
                        let variant_types: Vec<syn::Type> = tuples
                            .iter()
                            .map(|t| {
                                syn::parse_str(idl_ty_to_rust_ty(t).as_str()).expect("valid type")
                            })
                            .collect();

                        variant_ts = quote! {
                            #variant_ts
                            #variant_name(#(#variant_types),*),
                        };
                    }
                    None => {
                        variant_ts = quote! {
                            #variant_ts

                            #variant_name,

                        }
                    }
                }
            }
            quote! {
                #[derive(Clone, Debug, PartialEq, AnchorDeserialize, AnchorSerialize, Serialize, Deserialize)]
                pub enum #type_name {
                    #variant_ts
                }
            }
        }
        IdlTypeDefinitionTy::Struct { ref fields } => {
            let fields: Vec<TokenStream> = fields.iter().map(field_to_token_stream).collect();
            quote! {
                #[derive(Clone, Debug, PartialEq, AnchorDeserialize, AnchorSerialize, Serialize, Deserialize)]
                pub struct #type_name  {
                    #(#fields)*
                }
            }
        }
    };

    res
}

/// Converts an [IdlType] to a [String] of the Rust representation.
fn idl_ty_to_rust_ty(ty: &IdlType) -> String {
    match ty {
        IdlType::Bool => "bool".to_string(),
        IdlType::U8 => "u8".to_string(),
        IdlType::I8 => "i8".to_string(),
        IdlType::U16 => "u16".to_string(),
        IdlType::I16 => "i16".to_string(),
        IdlType::U32 => "u32".to_string(),
        IdlType::I32 => "i32".to_string(),
        IdlType::F32 => "f32".to_string(),
        IdlType::U64 => "u64".to_string(),
        IdlType::I64 => "i64".to_string(),
        IdlType::F64 => "f64".to_string(),
        IdlType::U128 => "u128".to_string(),
        IdlType::I128 => "i128".to_string(),
        IdlType::Bytes => "Vec<u8>".to_string(),
        IdlType::String => "String".to_string(),
        IdlType::PublicKey => "Pubkey".to_string(),
        IdlType::Option(inner) => format!("Option<{}>", idl_ty_to_rust_ty(inner)),
        IdlType::Vec(inner) => format!("Vec<{}>", idl_ty_to_rust_ty(inner)),
        IdlType::Array(ty, size) => format!("[{}; {}]", idl_ty_to_rust_ty(ty), size),
        IdlType::Defined(name) => name.to_string(),
        // https://github.com/coral-xyz/anchor/blob/9d947cb26b693e85e1fd26072bb046ff8f95bdcf/cli/src/lib.rs#L2459
        IdlType::U256 => unimplemented!("upon completion of u256 IDL standard"),
        IdlType::I256 => unimplemented!("upon completion of i256 IDL standard"),
    }
}

fn field_to_token_stream(f: &IdlField) -> TokenStream {
    let name = syn::Ident::new(f.name.as_str(), Span::call_site());
    let ty_str = idl_ty_to_rust_ty(&f.ty);
    let ty: syn::Type = syn::parse_str(ty_str.as_str()).unwrap();

    // TODO: quick hack (should ignore all arrays > 32)
    // arrays with len > 32 do not implement important traits e.g PartialEq, Serialize, etc.
    // in drift case the field is inconsequential 'padding' and can be safely ignored
    if ty_str.as_str() == "[u8; 48]" {
        TokenStream::new()
    } else {
        quote! {
            #name: #ty,
        }
    }
}
