use crate::SerdeErr;

pub struct BitReader {
    state: BitReaderState,
    buffer: Box<[u8]>,
}

impl BitReader {
    pub fn new(buffer: Box<[u8]>) -> Self {
        Self {
            state: BitReaderState {
                scratch: 0,
                scratch_index: 0,
                buffer_index: 0,
            },
            buffer,
        }
    }

    pub fn bytes_len(&self) -> usize {
        self.buffer.len()
    }

    pub fn read_bit(&mut self) -> Result<bool, SerdeErr> {
        if self.state.scratch_index == 0 {
            if self.state.buffer_index == self.buffer.len() {
                return Err(SerdeErr);
            }

            self.state.scratch = self.buffer[self.state.buffer_index];

            self.state.buffer_index += 1;
            self.state.scratch_index += 8;
        }

        let value = self.state.scratch & 1;

        self.state.scratch >>= 1;

        self.state.scratch_index -= 1;

        Ok(value != 0)
    }

    pub(crate) fn read_byte(&mut self) -> Result<u8, SerdeErr> {
        let mut output = 0;
        for _ in 0..7 {
            if self.read_bit()? {
                output |= 128;
            }
            output >>= 1;
        }
        if self.read_bit()? {
            output |= 128;
        }
        Ok(output)
    }
}

// BitReaderState
#[derive(Copy, Clone)]
struct BitReaderState {
    scratch: u8,
    scratch_index: u8,
    buffer_index: usize,
}
