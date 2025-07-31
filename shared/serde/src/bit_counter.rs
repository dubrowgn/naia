use crate::BitWrite;

pub struct BitCounter {
	bits: u32,
	capacity_bits: u32,
}

impl BitCounter {
    pub fn new(capacity_bits: u32) -> Self {
        Self {
			bits: 0,
			capacity_bits,
        }
    }

	pub fn overflowed(&self) -> bool { self.bits > self.capacity_bits }
	pub fn bits_needed(&self) -> u32 { self.bits }
}

impl BitWrite for BitCounter {
	fn write_bit(&mut self, _bit: bool) { self.bits += 1 }
	fn write_byte(&mut self, _byte: u8) { self.bits += 8 }
}
