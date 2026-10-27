use thiserror::Error;

/// Errors produced while encoding, decoding or parsing canonical types.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TypesError {
    /// The scalar does not match the declared logical type.
    #[error("unencodable value for type {0:?}")]
    Unencodable(&'static str),
    /// A serialized logical type tag is not recognized.
    #[error("unknown logical type tag {0}")]
    UnknownTypeTag(u8),
    /// The input bytes do not form a valid encoded value.
    #[error("malformed key bytes")]
    MalformedKey,
}
