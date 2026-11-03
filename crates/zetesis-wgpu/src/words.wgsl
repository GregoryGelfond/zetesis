// Position of one bit in an LSB-first array of 32-bit words.
// Each caller owns the row base, word access and synchronization.
struct PackedBit { word: u32, mask: u32 }

fn packed_bit(index: u32) -> PackedBit {
    return PackedBit(index / 32u, 1u << (index % 32u));
}
