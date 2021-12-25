use alloc::vec::Vec;
use alloc::vec;

pub struct BitField {
	values: Vec<u16>,
}

impl BitField {
	pub fn new(len: u16) -> BitField {
		BitField { values: vec![0, len] }
	}

	pub fn set(&mut self, i: u16) {
		let index = i / 32 | 0;
		let bit = i % 32;

		self.values[index as usize] |= 1 << bit;
	}


	pub fn get(&mut self, i: u16) -> bool {
		let index = i / 32 | 0;
		let bit = i % 32;
		return (self.values[index as usize] & 1 << bit) != 0;
	}

	pub fn unset(&mut self, i: u16) {
		let index = i / 32 | 0;
		let bit = i % 32;

		self.values[index as usize] &= !(1 << bit);
	}

	pub fn get_value(&mut self) -> u16 {
		self.values[0]
	}
}