/// The error message when failing to serialize/deserialize to/from the bit
/// stream.
#[derive(Clone, PartialEq)]
pub struct SerdeErr;

pub type SerdeResult<T> = Result<T, SerdeErr>;

impl std::fmt::Debug for SerdeErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Bin deserialize error",)
    }
}

impl std::fmt::Display for SerdeErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}

impl std::error::Error for SerdeErr {}
