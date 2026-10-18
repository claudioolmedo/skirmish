//! Hand-rolled 64-bit FNV-1a hasher for the per-tick state hash.
//!
//! Written from scratch for Skirmish. FNV-1a is a public-domain,
//! platform-independent hash: the same byte stream always yields the same
//! 64-bit value on every CPU, OS, and (crucially) on both native and
//! WebAssembly targets. The sim feeds it a *canonical* serialization of the
//! full state — tick counter, RNG words, and every entity field as raw bits —
//! so any nondeterminism anywhere in the sim changes the hash.

/// 64-bit FNV-1a hasher over an explicit, canonical byte stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fnv1a64(u64);

impl Fnv1a64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0100_0000_01b3;

    pub fn new() -> Self {
        Fnv1a64(Self::OFFSET_BASIS)
    }

    #[inline]
    pub fn write_u8(&mut self, byte: u8) {
        self.0 ^= byte as u64;
        self.0 = self.0.wrapping_mul(Self::PRIME);
    }

    pub fn write_u32(&mut self, value: u32) {
        for b in value.to_le_bytes() {
            self.write_u8(b);
        }
    }

    pub fn write_u64(&mut self, value: u64) {
        for b in value.to_le_bytes() {
            self.write_u8(b);
        }
    }

    pub fn write_i32(&mut self, value: i32) {
        self.write_u32(value as u32);
    }

    /// Feed a whole byte slice (M7): the slice length as a u64, then
    /// the bytes in 8-byte little-endian groups (the final group
    /// zero-padded), one multiply per group. The grid and fog layers
    /// are 14,400 bytes each at the 120×120 map size — feeding them
    /// byte-at-a-time would make the per-tick hash ~8× more expensive
    /// for no determinism gain: the packing is explicit little-endian
    /// assembly, identical on every platform, and the length prefix
    /// keeps padded tails unambiguous.
    pub fn write_bytes_chunked(&mut self, bytes: &[u8]) {
        self.write_u64(bytes.len() as u64);
        let mut chunks = bytes.chunks_exact(8);
        for c in &mut chunks {
            self.0 ^= u64::from_le_bytes(c.try_into().expect("8-byte chunk"));
            self.0 = self.0.wrapping_mul(Self::PRIME);
        }
        let rem = chunks.remainder();
        if !rem.is_empty() {
            let mut buf = [0u8; 8];
            buf[..rem.len()].copy_from_slice(rem);
            self.0 ^= u64::from_le_bytes(buf);
            self.0 = self.0.wrapping_mul(Self::PRIME);
        }
    }

    pub fn finish(&self) -> u64 {
        self.0
    }

    pub fn finish_hex(&self) -> String {
        format!("{:016x}", self.0)
    }
}

impl Default for Fnv1a64 {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_hash_is_offset_basis() {
        assert_eq!(Fnv1a64::new().finish(), Fnv1a64::OFFSET_BASIS);
    }

    #[test]
    fn known_vector() {
        // FNV-1a 64 test vector for the single byte 0x61 ('a').
        let mut h = Fnv1a64::new();
        h.write_u8(0x61);
        assert_eq!(h.finish(), 0xaf63_dc4c_8601_ec8c);
    }

    #[test]
    fn byte_order_is_canonical() {
        let mut a = Fnv1a64::new();
        a.write_u32(0x01020304);
        let mut b = Fnv1a64::new();
        for byte in [0x04u8, 0x03, 0x02, 0x01] {
            b.write_u8(byte);
        }
        assert_eq!(a.finish(), b.finish());
    }
}
