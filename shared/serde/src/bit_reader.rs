use crate::SerdeErr;

pub struct BitReader {
	bit_offset: u8,
    buffer: Box<[u8]>,
	buffer_index: usize,
}

impl BitReader {
    pub fn new(buffer: Box<[u8]>) -> Self {
        Self {
			bit_offset: 0,
            buffer,
			buffer_index: 0,
        }
    }

    pub fn bytes_len(&self) -> usize { self.buffer.len() }

    pub fn read_bit(&mut self) -> Result<bool, SerdeErr> {
		if self.buffer_index == self.buffer.len() {
			return Err(SerdeErr);
		}

		let mask = 1 << (7 - self.bit_offset);
		let bit = (self.buffer[self.buffer_index] & mask) != 0;

		self.bit_offset += 1;
		if self.bit_offset == 8 {
			self.buffer_index += 1;
			self.bit_offset = 0;
		}

		Ok(bit)
    }

    pub fn read_byte(&mut self) -> Result<u8, SerdeErr> {
		let bits_left = 8 * (self.buffer.len() - self.buffer_index) - self.bit_offset as usize;
		if bits_left < 8 {
			return Err(SerdeErr);
		}

		let mut byte = self.buffer[self.buffer_index];
		self.buffer_index += 1;

		if self.bit_offset != 0 {
			byte <<= self.bit_offset;
			byte |= self.buffer[self.buffer_index] >> (8 - self.bit_offset);
		}

        Ok(byte)
    }
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn read_mixed() {
		let bin = [0b1011_1000, 0b1001_1010];
		let mut reader = BitReader::new(bin.into());
		assert_eq!(reader.read_bit(), Ok(true));
		assert_eq!(reader.read_bit(), Ok(false));
		assert_eq!(reader.read_bit(), Ok(true));
		assert_eq!(reader.read_byte(), Ok(0b1100_0100));
		assert_eq!(reader.read_byte(), Err(SerdeErr));
		assert_eq!(reader.read_bit(), Ok(true));
		assert_eq!(reader.read_bit(), Ok(true));
		assert_eq!(reader.read_bit(), Ok(false));
		assert_eq!(reader.read_bit(), Ok(true));
		assert_eq!(reader.read_bit(), Ok(false));
		assert_eq!(reader.read_bit(), Err(SerdeErr));
	}
}