//! Colored references, as in ZGC (D69): a stored reference carries a color in its top bits, saying which collection
//! it's up to date with.
//!
//! A reference is the object's address with one of three color bits set. One color is good at any time; a reference
//! of another color may point where its object used to be, so the load barrier sends it to the slow path, which
//! finds the object through the forwarding table and heals the reference. The null reference, 0, has no color and
//! is always good.

/// The three color bits, one of which every non-null reference carries.
pub const COLOR_BITS: u64 = 0b111 << 60;

/// The address part of a reference.
pub const ADDRESS_BITS: u64 = (1 << 60) - 1;

/// The first collection's good color.
pub const FIRST_COLOR: u64 = 0b001 << 60;

/// The color after `color`, cycling through the three.
pub fn next_color(color: u64) -> u64 {
    let next = (color << 1) & COLOR_BITS;
    if next == 0 { FIRST_COLOR } else { next }
}

/// The colors that aren't `good`: a reference with any of them set needs the slow path.
pub fn bad_mask(good: u64) -> u64 {
    COLOR_BITS & !good
}

/// The object's address a reference points to.
pub fn address(reference: u64) -> usize {
    (reference & ADDRESS_BITS) as usize
}

/// A reference to `address` with `color`.
pub fn colored(address: usize, color: u64) -> u64 {
    if address == 0 {
        0
    } else {
        address as u64 | color
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_cycle_through_three() {
        let second = next_color(FIRST_COLOR);
        let third = next_color(second);
        assert_eq!(next_color(third), FIRST_COLOR);
        assert!(FIRST_COLOR != second && second != third && third != FIRST_COLOR);
    }

    #[test]
    fn only_a_good_reference_passes_the_barrier() {
        let good = next_color(FIRST_COLOR);
        let mask = bad_mask(good);
        assert_eq!(colored(0x1000, good) & mask, 0);
        assert_ne!(colored(0x1000, FIRST_COLOR) & mask, 0);
        assert_ne!(colored(0x1000, next_color(good)) & mask, 0);
        assert_eq!(colored(0, FIRST_COLOR), 0);
        assert_eq!(address(colored(0x1234, good)), 0x1234);
    }
}
