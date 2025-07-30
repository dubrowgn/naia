
pub use naia_serde_derive::{
    Serde, SerdeInternal,
};

mod bit_counter;
mod bit_reader;
mod bit_writer;
mod constants;
mod error;
mod impls;
mod integer;
mod serde;

pub use bit_counter::BitCounter;
pub use bit_reader::BitReader;
pub use bit_writer::{BitWrite, BitWriter};
pub use constants::{MTU_SIZE_BITS, MTU_SIZE_BYTES};
pub use error::{SerdeErr, SerdeResult};
pub use integer::{
    SerdeIntegerConversion, SignedInteger, SignedVariableInteger, UnsignedInteger,
    UnsignedVariableInteger,
};
pub use serde::{
    ConstBitLength, Serde, Serde as SerdeInternal,
};
