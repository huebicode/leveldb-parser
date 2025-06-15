const MASK_DELTA: u32 = 0xa282ead8;
pub fn unmask_crc32c(masked_crc: u32) -> u32 {
    let rot = masked_crc.wrapping_sub(MASK_DELTA);
    (rot >> 17) | (rot << 15)
}

pub fn decode_varint32(bytes: &[u8]) -> u32 {
    let mut result: u32 = 0;
    let mut shift: u32 = 0;

    for &byte in bytes {
        result |= ((byte & 0x7F) as u32) << shift;
        // break if continuation bit is not set
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
        if shift >= 32 {
            panic!("Varint32 overflow");
        }
    }
    result
}

pub fn decode_varint64(bytes: &[u8]) -> u64 {
    let mut result: u64 = 0;
    let mut shift: u64 = 0;

    for &byte in bytes {
        result |= ((byte & 0x7F) as u64) << shift;
        // break if continuation bit is not set
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
        if shift >= 64 {
            panic!("Varint64 overflow");
        }
    }
    result
}
