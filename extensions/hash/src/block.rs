//! Feeding a message to a block hash: the full blocks as they come, then the
//! tail padded the Merkle-Damgard way (a 0x80 byte, zeros, the message length
//! in bits) in one or two more blocks.

/// How the padded length is written.
#[derive(Clone, Copy)]
pub enum Length {
    /// 64 bits, little-endian: MD5.
    Little64,
    /// 64 bits, big-endian: SHA-1 and SHA-256.
    Big64,
    /// 128 bits, big-endian: SHA-512.
    Big128,
}

/// Calls `f` for every block of `data` once padded. `N` is the block size in
/// bytes, 64 or 128.
pub fn feed<const N: usize>(data: &[u8], length: Length, mut f: impl FnMut(&[u8; N])) {
    let (blocks, rest) = data.as_chunks::<N>();
    for block in blocks {
        f(block);
    }
    let len_bytes = match length {
        Length::Little64 | Length::Big64 => 8,
        Length::Big128 => 16,
    };
    // The tail needs the remainder, one 0x80, and the length. Two blocks
    // when that does not fit in one.
    let blocks = if rest.len() + 1 + len_bytes <= N {
        1
    } else {
        2
    };
    let mut tail = [0u8; 256];
    let tail = &mut tail[..N * blocks];
    tail[..rest.len()].copy_from_slice(rest);
    tail[rest.len()] = 0x80;
    let bits = (data.len() as u128).wrapping_mul(8);
    let end = tail.len();
    match length {
        Length::Little64 => {
            tail[end - 8..].copy_from_slice(&(bits as u64).to_le_bytes());
        }
        Length::Big64 => {
            tail[end - 8..].copy_from_slice(&(bits as u64).to_be_bytes());
        }
        Length::Big128 => {
            tail[end - 16..].copy_from_slice(&bits.to_be_bytes());
        }
    }
    for block in tail.as_chunks::<N>().0 {
        f(block);
    }
}

/// Lowercase hex of `bytes`.
pub fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(DIGITS[usize::from(b >> 4)] as char);
        out.push(DIGITS[usize::from(b & 0x0f)] as char);
    }
    out
}
