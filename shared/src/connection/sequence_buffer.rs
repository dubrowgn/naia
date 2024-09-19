use crate::SeqNum;

/// Collection to store data of any kind.
pub struct SequenceBuffer {
    sequence_num: SeqNum,
    entry_sequences: Box<[Option<SeqNum>]>,
}

impl SequenceBuffer {
    /// Creates a SequenceBuffer with a desired capacity.
    pub fn with_capacity(size: u16) -> Self {
        Self {
            sequence_num: SeqNum::ZERO,
            entry_sequences: vec![None; size as usize].into_boxed_slice(),
        }
    }

    /// Returns the most recently stored sequence number.
    pub fn sequence_num(&self) -> SeqNum {
        self.sequence_num
    }

    /// Inserts the entry data into the sequence buffer. If the requested
    /// sequence number is "too old", the entry will not be inserted and will
    /// return false
    pub fn set(&mut self, sequence_num: SeqNum) -> bool {
        // sequence number is too old to insert into the buffer
        if sequence_num < self.sequence_num - self.entry_sequences.len() as u16 {
            return false;
        }

        self.advance_sequence(sequence_num);

        let index = self.index(sequence_num);
        self.entry_sequences[index] = Some(sequence_num);

        true
    }

    /// Returns whether or not we have previously inserted an entry for the
    /// given sequence number.
    pub fn is_set(&self, sequence_num: SeqNum) -> bool {
        let index = self.index(sequence_num);
        if let Some(s) = self.entry_sequences[index] {
            return s == sequence_num;
        }
        false
    }

    /// Removes an entry from the sequence buffer
    fn unset(&mut self, sequence_num: SeqNum) {
        if self.is_set(sequence_num) {
            let index = self.index(sequence_num);
            self.entry_sequences[index] = None;
        }
    }

    // Advances the sequence number while removing older entries.
    fn advance_sequence(&mut self, sequence_num: SeqNum) {
        if sequence_num >= self.sequence_num {
            self.remove_entries(sequence_num);
            self.sequence_num = sequence_num + 1;
        }
    }

    fn remove_entries(&mut self, sequence_num: SeqNum) {
        let start_sequence = self.sequence_num.0 as u32;
		let mut finish_sequence = sequence_num.0 as u32;
        if finish_sequence < start_sequence {
            finish_sequence += 65536;
        }

        if finish_sequence - start_sequence < self.entry_sequences.len() as u32 {
            for sequence in start_sequence..=finish_sequence {
                self.unset((sequence as u16).into());
            }
        } else {
            for index in 0..self.entry_sequences.len() {
                self.entry_sequences[index] = None;
            }
        }
    }

    // Generates an index for use in `entry_sequences`.
    fn index(&self, sequence: SeqNum) -> usize {
        sequence.0 as usize % self.entry_sequences.len()
    }
}
