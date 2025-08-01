use crate::{
    constants::{MTU_SIZE_BITS, MTU_SIZE_BYTES},
    BitCounter, OutgoingPacket,
};

pub trait BitWrite {
    fn write_bit(&mut self, bit: bool);
    fn write_byte(&mut self, byte: u8);
}

pub struct BitWriter {
	bit_offset: u8,
    buffer: [u8; MTU_SIZE_BYTES],
    buffer_index: usize,
	capacity_bits: u32,
}

impl BitWriter {
    pub fn new() -> Self { Self::with_capacity(MTU_SIZE_BITS) }

    pub fn with_capacity(capacity_bits: u32) -> Self {
        Self {
			bit_offset: 0,
            buffer: [0; MTU_SIZE_BYTES],
            buffer_index: 0,
            capacity_bits,
        }
    }

	fn size(&self) -> usize { self.buffer_index + (self.bit_offset > 0) as usize }

    pub fn to_packet(&self) -> OutgoingPacket {
        OutgoingPacket::new(self.size(), self.buffer)
    }

    pub fn to_bytes(&self) -> Box<[u8]> { Box::from(&self.buffer[0..self.size()]) }

    pub fn counter(&self) -> BitCounter { BitCounter::new(self.capacity_bits) }

    pub fn reserve_bits(&mut self, bits: u32) {
        self.capacity_bits -= bits;
    }

    pub fn release_bits(&mut self, bits: u32) {
        self.capacity_bits += bits;
    }

    pub fn bits_free(&self) -> u32 { self.capacity_bits }
}

impl BitWrite for BitWriter {
    fn write_bit(&mut self, bit: bool) {
        if self.capacity_bits == 0 || self.buffer_index == self.buffer.len() {
            panic!("Write overflow!");
        }
		self.capacity_bits -= 1;

		let mask = (bit as u8) << (7 - self.bit_offset);
		self.buffer[self.buffer_index] |= mask;

        self.bit_offset += 1;
        if self.bit_offset == 8 {
            self.buffer_index += 1;
            self.bit_offset = 0;
        }
    }

	fn write_byte(&mut self, byte: u8) {
		let bits_left = 8 * (self.buffer.len() - self.buffer_index) - self.bit_offset as usize;
		if bits_left < 8 || self.capacity_bits < 8 {
			panic!("Write overflow!");
		}
		self.capacity_bits -= 8;

		self.buffer[self.buffer_index] |= byte >> self.bit_offset;
		self.buffer_index += 1;

		if self.bit_offset != 0 {
			self.buffer[self.buffer_index] |= byte << (8 - self.bit_offset);
		}
    }
}

#[cfg(test)]
mod tests {
	use crate::{bit_reader::BitReader, Serde};
	use super::*;

    #[test]
    fn read_write_bits() {
		let bits = [
			false, true, false, true, true, false, false, false,
			true, false, true, true, true, false, true, true,
		];

		let mut writer = BitWriter::new();
		for bit in bits {
			writer.write_bit(bit);
		}

		let mut reader = BitReader::new(writer.to_bytes());
		for bit in bits {
			assert_eq!(reader.read_bit(), Ok(bit));
		}
    }

    #[test]
    fn read_write_bytes() {
		let bytes = [48, 151, 62, 34, 2];

        let mut writer = BitWriter::new();
		for byte in bytes {
			writer.write_byte(byte);
		}
		let buffer = writer.to_bytes();

		// ensure bit order is preserved
		for (i, byte) in bytes.iter().enumerate() {
			assert_eq!(buffer[i], *byte);
		}

		let mut reader = BitReader::new(buffer);
		for byte in bytes {
			assert_eq!(reader.read_byte(), Ok(byte));
		}
    }

	#[test]
	fn read_write_mixed() {
		let val16 = 12345u16;
		let val32 = 123456789u32;

		let mut writer = BitWriter::new();
		writer.write_bit(true);
		val16.ser(&mut writer);
		writer.write_bit(false);
		val32.ser(&mut writer);
		writer.write_bit(true);

		let mut reader = BitReader::new(writer.to_bytes());
		assert_eq!(reader.read_bit(), Ok(true));
		assert_eq!(u16::de(&mut reader), Ok(val16));
		assert_eq!(reader.read_bit(), Ok(false));
		assert_eq!(u32::de(&mut reader), Ok(val32));
		assert_eq!(reader.read_bit(), Ok(true));
	}

	#[test]
	fn counter() {
		let mut writer = BitWriter::new();

		let counter = writer.counter();
		assert_eq!(counter.bits_needed(), 0);
		assert!(!counter.overflowed());

		writer.write_bit(true);
		37u32.ser(&mut writer);
		writer.write_bit(false);

		let mut counter = writer.counter();
		assert_eq!(counter.bits_needed(), 0);
		assert!(!counter.overflowed());
		counter.write_bit(true);
		37u32.ser(&mut counter);
		counter.write_bit(false);
		assert_eq!(counter.bits_needed(), 34);
		assert!(!counter.overflowed());
	}
}
