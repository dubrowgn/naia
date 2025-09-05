use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use syn::{DataStruct, Fields};

pub fn derive_serde_unit_struct(
	data_struct: &DataStruct,
	struct_name: &Ident,
	serde_crate_name: TokenStream,
) -> TokenStream {
	assert_eq!(data_struct.fields, Fields::Unit);

	let module_name = format!("define_{}", struct_name.to_string().to_lowercase());
	let module = Ident::new(module_name.as_str(), Span::call_site());

	quote! {
		mod #module {
			use #serde_crate_name::{BitReader, BitWrite, Serde, SerdeResult};
			use super::#struct_name;

			impl Serde for #struct_name {
				fn ser(&self, writer: &mut dyn BitWrite) { /* no-op */ }
				fn de(reader: &mut BitReader) -> SerdeResult<Self> { Ok(Self) }
				fn bit_length(&self) -> u32 { 0 }
			}
		}
	}
}
