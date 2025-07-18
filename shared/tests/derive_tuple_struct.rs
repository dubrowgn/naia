mod some_struct {
    use naia_shared::Serde;

    #[derive(Clone, Debug, PartialEq, Serde)]
    pub struct SomeStruct(pub String, pub i16, pub bool);
}

use naia_shared::{BitReader, BitWriter, Serde};
use some_struct::SomeStruct;

#[test]
fn read_write_tuple_struct() {
    // Write
    let mut writer = BitWriter::new();

    let in_1 = SomeStruct("Hello world!".to_string(), 42, true);
    let in_2 = SomeStruct("Goodbye world!".to_string(), -42, false);

    in_1.ser(&mut writer);
    in_2.ser(&mut writer);

    let bytes = writer.to_bytes();

    // Read

    let mut reader = BitReader::new(bytes);

    let out_1 = Serde::de(&mut reader).unwrap();
    let out_2 = Serde::de(&mut reader).unwrap();

    assert_eq!(in_1, out_1);
    assert_eq!(in_2, out_2);
}
