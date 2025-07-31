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

		let mask = 1 << self.bit_offset;
		let bit = (self.buffer[self.buffer_index] & mask) != 0;

		self.bit_offset += 1;
		if self.bit_offset == 8 {
			self.buffer_index += 1;
			self.bit_offset = 0;
		}

		Ok(bit)
    }

    pub(crate) fn read_byte(&mut self) -> Result<u8, SerdeErr> {
        let mut byte = 0;
        for _ in 0..8 {
            byte <<= 1;
			byte |= self.read_bit()? as u8;
        }
        Ok(byte)
    }
}
