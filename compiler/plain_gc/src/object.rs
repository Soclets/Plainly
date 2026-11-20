//! How an object is laid out, so the collector can find every reference inside it.
//!
//! An object is one header word, then its payload, padded to a whole number of words. The header holds the object's
//! kind in its top byte and its payload's length in bytes in the low 48 bits. A reference to an object is its
//! address, the address of its header.

/// The size of a word: a header, or a reference.
pub const WORD: usize = 8;

/// The longest payload a header can describe.
pub const MAX_LENGTH: usize = (1 << 48) - 1;

const KIND_SHIFT: u32 = 56;

/// What an object holds, which tells the collector where its references are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectKind {
    /// Bytes with no references in them, like `Text`.
    Bytes,
    /// Nothing but references, one per word: a list's items, for example.
    References,
}

impl ObjectKind {
    fn number(self) -> u64 {
        match self {
            ObjectKind::Bytes => 1,
            ObjectKind::References => 2,
        }
    }

    fn from_number(number: u64) -> Option<ObjectKind> {
        match number {
            1 => Some(ObjectKind::Bytes),
            2 => Some(ObjectKind::References),
            _ => None,
        }
    }
}

/// The header word for an object of `kind` with a payload of `length` bytes.
pub fn header(kind: ObjectKind, length: usize) -> u64 {
    (kind.number() << KIND_SHIFT) | (length as u64 & MAX_LENGTH as u64)
}

/// The kind and payload length a header describes, or `None` if it isn't a header at all.
pub fn read_header(header: u64) -> Option<(ObjectKind, usize)> {
    let kind = ObjectKind::from_number(header >> KIND_SHIFT)?;
    Some((kind, (header & MAX_LENGTH as u64) as usize))
}

/// The whole size of an object with a payload of `length` bytes: its header and its payload, padded to words.
pub fn object_size(length: usize) -> usize {
    WORD + length.div_ceil(WORD) * WORD
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_header_gives_back_its_kind_and_length() {
        for kind in [ObjectKind::Bytes, ObjectKind::References] {
            for length in [0, 1, 7, 8, 9, 1000, MAX_LENGTH] {
                assert_eq!(read_header(header(kind, length)), Some((kind, length)));
            }
        }
        assert_eq!(read_header(0), None);
    }

    #[test]
    fn objects_are_whole_words() {
        assert_eq!(object_size(0), 8);
        assert_eq!(object_size(1), 16);
        assert_eq!(object_size(8), 16);
        assert_eq!(object_size(9), 24);
    }
}
