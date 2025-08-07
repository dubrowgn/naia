use crate::{
    bit_reader::BitReader,
    bit_writer::BitWrite,
    error::SerdeErr,
    serde::{ConstBitLength, Serde},
};

// Unit //

impl Serde for () {
    fn ser(&self, _: &mut dyn BitWrite) {}

    fn de(_: &mut BitReader) -> Result<Self, SerdeErr> {
        Ok(())
    }

    fn bit_length(&self) -> u32 {
        <Self as ConstBitLength>::const_bit_length()
    }
}

impl ConstBitLength for () {
    fn const_bit_length() -> u32 {
        0
    }
}

// tests

#[cfg(test)]
mod unit_tests {
    use crate::{bit_reader::BitReader, bit_writer::BitWriter, serde::Serde};

    #[test]
    fn read_write() {
        // Write
        let mut writer = BitWriter::new();

        let in_unit = ();

        in_unit.ser(&mut writer);

        //Read
        let mut reader = BitReader::from_slice(writer.slice());

        let out_unit = Serde::de(&mut reader).unwrap();

        assert_eq!(in_unit, out_unit);
    }
}

// Boolean //

impl Serde for bool {
    fn ser(&self, writer: &mut dyn BitWrite) {
        writer.write_bit(*self);
    }

    fn de(reader: &mut BitReader) -> Result<Self, SerdeErr> {
        reader.read_bit()
    }

    fn bit_length(&self) -> u32 {
        <Self as ConstBitLength>::const_bit_length()
    }
}

impl ConstBitLength for bool {
    fn const_bit_length() -> u32 {
        1
    }
}

// tests

#[cfg(test)]
mod bool_tests {
    use crate::{bit_reader::BitReader, bit_writer::BitWriter, serde::Serde};

    #[test]
    fn read_write() {
        // Write
        let mut writer = BitWriter::new();

        let in_1 = true;
        let in_2 = false;

        in_1.ser(&mut writer);
        in_2.ser(&mut writer);

        //Read
        let mut reader = BitReader::from_slice(writer.slice());

        let out_1 = Serde::de(&mut reader).unwrap();
        let out_2 = Serde::de(&mut reader).unwrap();

        assert_eq!(in_1, out_1);
        assert_eq!(in_2, out_2);
    }
}

// Characters //

impl Serde for char {
    fn ser(&self, writer: &mut dyn BitWrite) {
        let u32char = *self as u32;
        let bytes = unsafe { std::mem::transmute::<&u32, &[u8; 4]>(&u32char) };
        for byte in bytes {
            writer.write_byte(*byte);
        }
    }

    fn de(reader: &mut BitReader) -> Result<Self, SerdeErr> {
        let mut bytes = [0_u8; 4];
        for byte in &mut bytes {
            *byte = reader.read_byte()?;
        }
        let mut container = [0_u32];
        unsafe {
            std::ptr::copy_nonoverlapping(
                bytes.as_ptr().offset(0_isize) as *const u32,
                container.as_mut_ptr() as *mut u32,
                1,
            )
        }

        if let Some(inner_char) = char::from_u32(container[0]) {
            Ok(inner_char)
        } else {
            Err(SerdeErr {})
        }
    }

    fn bit_length(&self) -> u32 {
        <Self as ConstBitLength>::const_bit_length()
    }
}

impl ConstBitLength for char {
    fn const_bit_length() -> u32 {
        <[u8; 4] as ConstBitLength>::const_bit_length()
    }
}

// tests

#[cfg(test)]
mod char_tests {
    use crate::{bit_reader::BitReader, bit_writer::BitWriter, serde::Serde};

    #[test]
    fn read_write() {
        // Write
        let mut writer = BitWriter::new();

        let in_1 = 'O';
        let in_2 = '!';

        in_1.ser(&mut writer);
        in_2.ser(&mut writer);

        //Read
        let mut reader = BitReader::from_slice(writer.slice());

        let out_1 = Serde::de(&mut reader).unwrap();
        let out_2 = Serde::de(&mut reader).unwrap();

        assert_eq!(in_1, out_1);
        assert_eq!(in_2, out_2);
    }
}

// Integers & Floating-point Numbers //

macro_rules! impl_serde_for {
    ($impl_type:ident) => {
        impl Serde for $impl_type {
            fn ser(&self, writer: &mut dyn BitWrite) {
                for byte in self.to_le_bytes() {
                    writer.write_byte(byte);
                }
            }

            fn de(reader: &mut BitReader) -> Result<$impl_type, SerdeErr> {
                const BYTES_LENGTH: usize = std::mem::size_of::<$impl_type>();
                let mut byte_array = [0_u8; BYTES_LENGTH];
                for index in 0..BYTES_LENGTH {
                    byte_array[index] = reader.read_byte()?;
                }
				Ok($impl_type::from_le_bytes(byte_array))
            }

            fn bit_length(&self) -> u32 {
                <Self as ConstBitLength>::const_bit_length()
            }
        }
        impl ConstBitLength for $impl_type {
            fn const_bit_length() -> u32 {
                const BYTES_LENGTH: u32 = std::mem::size_of::<$impl_type>() as u32;
                return BYTES_LENGTH * 8;
            }
        }
    };
}

// number primitives
impl_serde_for!(u8);
impl_serde_for!(u16);
impl_serde_for!(u32);
impl_serde_for!(u64);
impl_serde_for!(i8);
impl_serde_for!(i16);
impl_serde_for!(i32);
impl_serde_for!(i64);
impl_serde_for!(f32);
impl_serde_for!(f64);

impl ConstBitLength for isize {
    fn const_bit_length() -> u32 {
        <u64 as ConstBitLength>::const_bit_length()
    }
}

#[cfg(test)]
mod tests {
	use crate::{bit_reader::BitReader, bit_writer::BitWriter, serde::Serde};

	macro_rules! test_roundtrip {
		($impl_type:ident, $test_name:ident, $value:literal) => {
			#[test]
			fn $test_name() {
				let mut writer = BitWriter::new();
				$value.ser(&mut writer);

				let mut reader = BitReader::from_slice(writer.slice());
				assert_eq!($impl_type::de(&mut reader), Ok($value));
			}
		};
	}

    test_roundtrip!(u8, u8_roundtrip, 123u8);
    test_roundtrip!(u16, u16_roundtrip, 12345u16);
    test_roundtrip!(u32, u32_roundtrip, 1234567890u32);
    test_roundtrip!(u64, u64_roundtrip, 12345678901234567890u64);
    test_roundtrip!(i8, i8_roundtrip, -123i8);
    test_roundtrip!(i16, i16_roundtrip, -12345i16);
    test_roundtrip!(i32, i32_roundtrip, -1234567890i32);
    test_roundtrip!(i64, i64_roundtrip, -1234567890123456789i64);
    test_roundtrip!(f32, f32_roundtrip, 123.456f32);
    test_roundtrip!(f64, f64_roundtrip, 1234567890123456789.1234567890123456789f64);
}
