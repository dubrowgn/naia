//! # Naia Derive
//! Procedural macros to simplify implementation of Naia types

#![deny(trivial_casts, trivial_numeric_casts, unstable_features)]

use quote::quote;

mod channel;
mod message;
mod shared;

use channel::channel_impl;
use message::message_impl;

// Channel

/// Derives the Channel trait for a given struct
#[proc_macro_derive(Channel)]
pub fn channel_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    channel_impl(input)
}

// Message

/// Derives the Message trait for a given struct, for internal
#[proc_macro_derive(MessageInternal)]
pub fn message_derive_internal(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let shared_crate_name = quote! { crate };
    message_impl(input, shared_crate_name, false)
}

/// Derives the Message trait for a given struct, for FragmentedMessage
#[proc_macro_derive(MessageFragment)]
pub fn message_derive_fragment(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let shared_crate_name = quote! { crate };
    message_impl(input, shared_crate_name, true)
}

/// Derives the Message trait for a given struct
#[proc_macro_derive(Message)]
pub fn message_derive_shared(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let shared_crate_name = quote! { naia_shared };
    message_impl(input, shared_crate_name, false)
}
