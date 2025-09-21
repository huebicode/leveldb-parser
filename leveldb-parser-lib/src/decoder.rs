#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageKind {
    SessionStorage,
    LocalStorage,
    IndexedDb,
    Generic,
}

pub fn detect_storage_kind(path: &str) -> StorageKind {
    let lower = path.to_ascii_lowercase();
    match () {
        _ if lower.contains("local storage") => StorageKind::LocalStorage,
        _ if lower.contains("session storage") => StorageKind::SessionStorage,
        _ if lower.contains("indexeddb") => StorageKind::IndexedDb,
        _ => StorageKind::Generic,
    }
}

pub fn decode_kv(kind: StorageKind, key: &[u8], value: Option<&[u8]>) -> (String, String) {
    match kind {
        StorageKind::SessionStorage => {
            let k = bytes_to_utf8_lossy(key);
            let v = match value {
                Some(v_bytes) => {
                    if k.starts_with("map-") {
                        if let Some(s) = try_utf16le(v_bytes) {
                            s
                        } else {
                            bytes_to_utf8_lossy(v_bytes)
                        }
                    } else {
                        bytes_to_utf8_lossy(v_bytes)
                    }
                }
                None => String::new(),
            };
            (k, v)
        }
        StorageKind::LocalStorage => {
            let is_entry = key.starts_with(b"_");

            let k = if is_entry {
                decode_local_storage_entry(key)
            } else {
                bytes_to_latin1_hex_escaped(key)
            };
            let v = match value {
                Some(v_bytes) => {
                    if is_entry {
                        decode_local_storage_entry(v_bytes)
                    } else {
                        bytes_to_hex(v_bytes)
                    }
                }
                None => String::new(),
            };
            (k, v)
        }
        StorageKind::IndexedDb => {
            todo!()
        }
        StorageKind::Generic => {
            let k = bytes_to_utf8_lossy(key);
            let v = match value {
                Some(v_bytes) => bytes_to_utf8_lossy(v_bytes),
                None => String::new(),
            };
            (k, v)
        }
    }
}

fn decode_local_storage_entry(bytes: &[u8]) -> String {
    if bytes.starts_with(b"_") {
        // key
        let delim_pos = match bytes.iter().position(|&b| b == 0x00) {
            Some(p) => p,
            None => return bytes_to_latin1_hex_escaped(bytes), // fallback without delimiter
        };
        if delim_pos + 1 >= bytes.len() {
            // key prefix only
            return bytes_to_latin1_hex_escaped(bytes);
        }

        // decode key
        let key_prefix = &bytes[..delim_pos];
        let encoding_flag = bytes[delim_pos + 1];
        let key_entry = &bytes[delim_pos + 2..];

        let key = match encoding_flag {
            0x00 => {
                // UTF-16LE
                if let Some(entry) = try_utf16le(key_entry) {
                    format!("{} {}", bytes_to_latin1_hex_escaped(key_prefix), entry)
                } else {
                    bytes_to_latin1_hex_escaped(bytes) // fallback
                }
            }
            0x01 => {
                // Latin-1
                format!(
                    "{} {}",
                    bytes_to_latin1_hex_escaped(key_prefix),
                    bytes_to_latin1_hex_escaped(key_entry)
                )
            }
            _ => bytes_to_latin1_hex_escaped(bytes), // unknown flag => fallback
        };

        return key;
    } else if bytes.starts_with(&[0x00]) || bytes.starts_with(&[0x01]) {
        // value
        let encoding_flag = bytes[0];
        let value_entry = &bytes[1..];

        let value = match encoding_flag {
            0x00 => {
                // UTF-16LE
                if let Some(entry) = try_utf16le(value_entry) {
                    entry
                } else {
                    bytes_to_latin1_hex_escaped(value_entry) // fallback
                }
            }
            0x01 => {
                // Latin-1
                bytes_to_latin1_hex_escaped(value_entry)
            }
            _ => bytes_to_latin1_hex_escaped(value_entry), // unknown flag => fallback
        };

        return value;
    }

    bytes_to_latin1_hex_escaped(bytes) // fallback
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("\\x{:02X}", b)).collect()
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

fn bytes_to_latin1_hex_escaped(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| {
            if b.is_ascii_graphic() || b == 0x20 || b >= 0xA0 {
                (b as char).to_string()
            } else {
                format!("\\x{:02X}", b)
            }
        })
        .collect()
}

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
