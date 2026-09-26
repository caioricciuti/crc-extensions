//! CRC-32 as in IEEE 802.3, zlib and `cksum -o 3`: reflected polynomial
//! 0xEDB88320, initial value all ones, final complement.

/// The CRC-32 of `data`.
pub fn crc32(data: &[u8]) -> u32 {
    let table = table();
    let mut crc: u32 = 0xffff_ffff;
    for &byte in data {
        let index = usize::from(((crc ^ u32::from(byte)) & 0xff) as u8);
        crc = table[index] ^ (crc >> 8);
    }
    !crc
}

fn table() -> [u32; 256] {
    let mut table = [0u32; 256];
    for (i, entry) in table.iter_mut().enumerate() {
        let mut crc = i as u32;
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                0xedb8_8320 ^ (crc >> 1)
            } else {
                crc >> 1
            };
        }
        *entry = crc;
    }
    table
}
