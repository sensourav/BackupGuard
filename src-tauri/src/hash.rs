use std::io::Read;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::quickxor::QuickXorHash;
use crate::walker::open_read_only;

pub const CHUNK_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDigest {
    pub sha256_hex: String,
    pub quickxor_b64: String,
    pub bytes_read: u64,
}

/// Hash one file through a read-only handle. `buffer` is reused by the caller so a
/// large media file never sits in memory whole. The 8MB chunk keeps the buffer under 30MB.
pub fn hash_file(
    path: &Path,
    buffer: &mut [u8],
    mut on_bytes: impl FnMut(u64),
) -> Result<FileDigest, String> {
    if buffer.is_empty() {
        return Err("hash buffer is empty".to_string());
    }
    let mut file = open_read_only(path).map_err(|err| format!("{}: {err}", path.display()))?;
    let mut sha = Sha256::new();
    let mut quickxor = QuickXorHash::new();
    let mut total = 0u64;
    loop {
        let read = file
            .read(buffer)
            .map_err(|err| format!("{}: {err}", path.display()))?;
        if read == 0 {
            break;
        }
        sha.update(&buffer[..read]);
        quickxor.update(&buffer[..read]);
        total += read as u64;
        on_bytes(total);
    }
    Ok(FileDigest {
        sha256_hex: hex_encode(&sha.finalize()),
        quickxor_b64: quickxor.finish_base64(),
        bytes_read: total,
    })
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn sha256_of_abc_matches_the_known_digest() {
        let root = crate::walker::fixture_root("hash");
        fs::create_dir_all(&root).unwrap();
        let path = root.join("abc.txt");
        fs::write(&path, b"abc").unwrap();
        let before = fs::read(&path).unwrap();

        let mut buffer = vec![0u8; 8];
        let digest = hash_file(&path, &mut buffer, |_| {}).unwrap();
        assert_eq!(
            digest.sha256_hex,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(digest.bytes_read, 3);
        assert_eq!(fs::read(&path).unwrap(), before);
        let _ = fs::remove_dir_all(root);
    }
}
