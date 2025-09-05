use proc_macro2::{Ident, Span, TokenStream};
use quote::{format_ident, quote};
use syn::{DataStruct, Index};

#[allow(clippy::format_push_string)]
pub fn derive_serde_tuple_struct(
    struct_: &DataStruct,
    struct_name: &Ident,
    serde_crate_name: TokenStream,
) -> TokenStream {
    let mut ser_body = quote! {};
    let mut de_body = quote! {};
    let mut bit_length_body = quote! {};

    for (i, _) in struct_.fields.iter().enumerate() {
        let field_index = Index::from(i);
        ser_body = quote! {
            #ser_body
            self.#field_index.ser(writer);
        };
        de_body = quote! {
            #de_body
			Serde::de(reader)?,
        };
        bit_length_body = quote! {
            #bit_length_body
            output += self.#field_index.bit_length();
        };
    }

    let lowercase_struct_name = Ident::new(
        struct_name.to_string().to_lowercase().as_str(),
        Span::call_site(),
    );
    let module_name = format_ident!("define_{}", lowercase_struct_name);

    quote! {
        mod #module_name {
			use #serde_crate_name::{
				BitReader, BitWrite, ConstBitLength, Serde, SerdeResult,
			};
            use super::#struct_name;
            impl Serde for #struct_name {
                 fn ser(&self, writer: &mut dyn BitWrite) {
                    #ser_body
                 }
                 fn de(reader: &mut BitReader) -> SerdeResult<Self> {
					Ok(Self (
                        #de_body
					))
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
