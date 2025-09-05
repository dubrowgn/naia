use proc_macro2::{Ident, Span, TokenStream};
use quote::{format_ident, quote};
use syn::DataStruct;

#[allow(clippy::format_push_string)]
pub fn derive_serde_struct(
    struct_: &DataStruct,
    struct_name: &Ident,
    serde_crate_name: TokenStream,
) -> TokenStream {
    let mut ser_body = quote! {};
    let mut de_body = quote! {};
    let mut bit_length_body = quote! {};

    for field in &struct_.fields {
        let field_name = field.ident.as_ref().expect("expected field to have a name");
        ser_body = quote! {
            #ser_body
            self.#field_name.ser(writer);
        };
        de_body = quote! {
            #de_body
            #field_name: Serde::de(reader)?,
        };
        bit_length_body = quote! {
            #bit_length_body
            output += self.#field_name.bit_length();
        };
    }

    let lowercase_struct_name = Ident::new(
        struct_name.to_string().to_lowercase().as_str(),
        Span::call_site(),
    );
    let module_name = format_ident!("define_{}", lowercase_struct_name);

    quote! {
        mod #module_name {
			use #serde_crate_name::{BitReader, BitWrite, Serde, SerdeResult};
            use super::#struct_name;
            impl Serde for #struct_name {
                 fn ser(&self, writer: &mut dyn BitWrite) {
                    #ser_body
                 }
                 fn de(reader: &mut BitReader) -> SerdeResult<Self> {
                    Ok(Self {
                        #de_body
                    })
                 }
                fn bit_length(&self) -> u32 {
                    let mut output = 0;
                    #bit_length_body
                    output
                }
            }
        }
    }
}
