use crate::SerdeErr;

pub struct BitReader {
    buffer: Box<[u8]>,
	buffer_index: usize,
	scratch: u8,
	scratch_index: u8,
}

impl BitReader {
    pub fn new(buffer: Box<[u8]>) -> Self {
        Self {
            buffer,
			buffer_index: 0,
			scratch: 0,
			scratch_index: 0,
        }
    }

    pub fn bytes_len(&self) -> usize {
        self.buffer.len()
    }

    pub fn read_bit(&mut self) -> Result<bool, SerdeErr> {
        if self.scratch_index == 0 {
            if self.buffer_index == self.buffer.len() {
                return Err(SerdeErr);
            }

            self.scratch = self.buffer[self.buffer_index];

            self.buffer_index += 1;
            self.scratch_index += 8;
        }

        let value = self.scratch & 1;

        self.scratch >>= 1;

        self.scratch_index -= 1;

        Ok(value != 0)
    }

    pub(crate) fn read_byte(&mut self) -> Result<u8, SerdeErr> {
        let mut output = 0;
        for _ in 0..8 {
            output >>= 1;
            if self.read_bit()? {
                output |= 128;
            }
        }
        Ok(output)
    }
}
