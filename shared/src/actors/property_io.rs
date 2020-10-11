use nanoserde::{DeBin, SerBin};

use super::property::Property;

use crate::{packet_reader::PacketReader, wrapping_number::sequence_greater_than};

/// A Property that can read/write itself from/into incoming/outgoing packets
pub trait PropertyIo<T: Clone + DeBin + SerBin + PartialEq> {
    /// Writes contained value into outgoing byte stream
    fn write(&self, buffer: &mut Vec<u8>);
    /// Given a cursor into incoming packet data, updates the Property with the
    /// synced value
    fn read(&mut self, reader: &mut PacketReader);
    /// Given a cursor into incoming packet data, updates the Property with the
    /// synced value, but only if data is newer than the last data received
    fn read_seq(&mut self, reader: &mut PacketReader, packet_index: u16);
}

impl<T: Clone + DeBin + SerBin + PartialEq> PropertyIo<T> for Property<T> {
    fn write(&self, buffer: &mut Vec<u8>) {
        let encoded = &mut SerBin::serialize_bin(&self.inner);
        buffer.push(encoded.len() as u8);
        buffer.append(encoded);
    }

    fn read(&mut self, reader: &mut PacketReader) {
        let length = reader.read_u8();

        let buffer = reader.get_buffer();
        let cursor = reader.get_cursor();

        let start: usize = cursor.position() as usize;
        let end: usize = start + (length as usize);

        self.inner = DeBin::deserialize_bin(&buffer[start..end]).unwrap();

        cursor.set_position(end as u64);
    }

    fn read_seq(&mut self, reader: &mut PacketReader, packet_index: u16) {
        let length = reader.read_u8();

        let buffer = reader.get_buffer();
        let cursor = reader.get_cursor();

        let start: usize = cursor.position() as usize;
        let end: usize = start + (length as usize);

        if sequence_greater_than(packet_index, self.last_recv_index) {
            self.last_recv_index = packet_index;
            self.inner = DeBin::deserialize_bin(&buffer[start..end]).unwrap();
        }

        cursor.set_position(end as u64);
    }
}