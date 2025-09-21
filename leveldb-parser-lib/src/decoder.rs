#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageKind {
    LocalStorage,
    IndexedDb,
    Generic,
}

pub fn detect_storage_kind(path: &str) -> StorageKind {
    let lower = path.to_ascii_lowercase();
    if lower.contains("local storage") {
        println!("Detected LocalStorage from file path");
        StorageKind::LocalStorage
    } else if lower.contains("indexeddb") {
        println!("Detected IndexedDb from file path");
        StorageKind::IndexedDb
    } else {
        println!("Detected Generic storage from file path");
        StorageKind::Generic
    }
}

pub fn decode_storage_bytes(kind: StorageKind, bytes: &[u8]) -> String {
    match kind {
        StorageKind::LocalStorage => {
            if let Some(decoded) = decode_local_storage_bytes(bytes) {
                decoded
            } else {
                bytes_to_utf8_lossy(bytes)
            }
        }
        StorageKind::IndexedDb => {
            println!("Decoding as IndexedDb");
            let utf8 = bytes_to_utf8_lossy(bytes);
            if !utf8.is_empty() {
                utf8
            } else {
                bytes_to_ascii_with_hex(bytes)
            }
        }
        StorageKind::Generic => {
            println!("Decoding as Generic");
            bytes_to_ascii_with_hex(bytes)
        }
    }
}

fn decode_local_storage_bytes(bytes: &[u8]) -> Option<String> {
    // check if key
    if bytes.starts_with(b"_") {
        let delim_pos = bytes.iter().position(|&b| b == 0x00)?;
        if delim_pos + 1 >= bytes.len() {
            // key prefix only
            if let Ok(key_str) = std::str::from_utf8(&bytes[..delim_pos]) {
                return Some(key_str.to_string());
            } else {
                return None;
            }
        }

        // get the key
        let key_prefix = &bytes[..delim_pos];
        let flag = bytes[delim_pos + 1];
        let key_remainder = &bytes[delim_pos + 2..];

        let key = match flag {
            0x00 => {
                // UTF-16LE
                println!("Decoding as UTF-16LE");
                if let Some(s) = try_utf16le(key_remainder) {
                    format!("{} {}", bytes_to_utf8_lossy(key_prefix), s)
                } else {
                    return None; // TODO: fallback?
                }
            }
            0x01 => {
                // Latin-1
                println!("Decoding as Latin-1");
                format!(
                    "{} {}",
                    bytes_to_utf8_lossy(key_prefix),
                    bytes_to_latin1(key_remainder)
                )
            }
            _ => {
                // unknown flag => fallback
                bytes_to_utf8_lossy(bytes)
            }
        };

        return Some(key);
    } //TODO: check if value, starts with 0x00 or 0x01

    None
}

pub fn bytes_to_ascii(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| {
            if b.is_ascii_graphic() || b == 0x20 {
                b as char
            } else {
                '.'
            }
        })
        .collect()
}

pub fn bytes_to_ascii_with_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| {
            if b.is_ascii_graphic() || b == 0x20 {
                (b as char).to_string()
            } else {
                format!("\\x{:02X}", b)
            }
        })
        .collect()
}

fn bytes_to_latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

// pub fn bytes_to_latin1_with_hex(bytes: &[u8]) -> String {
//     bytes
//         .iter()
//         .map(|&b| {
//             if b.is_ascii_graphic() || b == 0x20 || b >= 0xA0 {
//                 (b as char).to_string()
//             } else {
//                 format!("\\x{:02X}", b)
//             }
//         })
//         .collect()
// }

pub fn bytes_to_utf8_lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .chars()
        .filter(|c| !c.is_control())
        .collect()
}

fn try_utf16le(bytes: &[u8]) -> Option<String> {
    if bytes.len() >= 2 && bytes.len() % 2 == 0 {
        let mut buf = Vec::with_capacity(bytes.len() / 2);
        for c in bytes.chunks(2) {
            buf.push(u16::from_le_bytes([c[0], c[1]]));
        }
        if let Ok(s) = String::from_utf16(&buf) {
            if s.chars().any(|c| !c.is_control()) {
                return Some(s);
            }
        }
    }
    None
}
