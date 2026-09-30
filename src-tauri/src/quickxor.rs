const BITS_IN_LAST_CELL: i32 = 32;
const SHIFT: i32 = 11;
const WIDTH_IN_BITS: i32 = 160;
const DATA_LEN: usize = ((WIDTH_IN_BITS - 1) / 64 + 1) as usize;

/// Microsoft QuickXorHash (160-bit width, shift 11). The digest is the 20-byte
/// value OneDrive returns as `quickXorHash`, before base64 encoding.
#[derive(Clone)]
pub struct QuickXorHash {
    data: [u64; DATA_LEN],
    length_so_far: i64,
    shift_so_far: i32,
}

impl QuickXorHash {
    pub fn new() -> Self {
        Self {
            data: [0; DATA_LEN],
            length_so_far: 0,
            shift_so_far: 0,
        }
    }

    pub fn update(&mut self, bytes: &[u8]) {
        let cb_size = bytes.len() as i32;
        let mut vector_array_index = self.shift_so_far / 64;
        let mut vector_offset = self.shift_so_far % 64;
        let iterations = cb_size.min(WIDTH_IN_BITS);

        for i in 0..iterations {
            let mut is_last_cell = vector_array_index == DATA_LEN as i32 - 1;
            let mut bits_in_vector_cell = if is_last_cell { BITS_IN_LAST_CELL } else { 64 };

            if vector_offset <= bits_in_vector_cell - 8 {
                let mut j = i;
                while j < cb_size {
                    self.data[vector_array_index as usize] ^=
                        (bytes[j as usize] as u64) << vector_offset;
                    j += WIDTH_IN_BITS;
                }
            } else {
                let index1 = vector_array_index as usize;
                let index2 = if is_last_cell {
                    0
                } else {
                    (vector_array_index as usize) + 1
                };
                let low = (bits_in_vector_cell - vector_offset) as u32;
                let mut j = i;
                while j < cb_size {
                    let value = bytes[j as usize] as u64;
                    self.data[index1] ^= value << vector_offset;
                    self.data[index2] ^= value >> low;
                    j += WIDTH_IN_BITS;
                }
            }

            vector_offset += SHIFT;
            while vector_offset >= bits_in_vector_cell {
                vector_array_index = if is_last_cell {
                    0
                } else {
                    vector_array_index + 1
                };
                vector_offset -= bits_in_vector_cell;
                is_last_cell = vector_array_index == DATA_LEN as i32 - 1;
                bits_in_vector_cell = if is_last_cell { BITS_IN_LAST_CELL } else { 64 };
            }
        }

        self.shift_so_far =
            (self.shift_so_far + SHIFT * (cb_size % WIDTH_IN_BITS)) % WIDTH_IN_BITS;
        self.length_so_far += cb_size as i64;
    }

    pub fn finish(&self) -> [u8; 20] {
        let mut rgb = [0u8; 20];
        for (index, value) in self.data.iter().enumerate().take(DATA_LEN - 1) {
            rgb[index * 8..index * 8 + 8].copy_from_slice(&value.to_le_bytes());
        }
        let offset = (DATA_LEN - 1) * 8;
        let last = self.data[DATA_LEN - 1].to_le_bytes();
        let tail = rgb.len() - offset;
        rgb[offset..].copy_from_slice(&last[..tail]);

        let start = rgb.len() - 8;
        for (index, byte) in rgb.iter_mut().enumerate().skip(start) {
            let shift = 8 * (index - start);
            *byte ^= ((self.length_so_far >> shift) & 0xff) as u8;
        }
        rgb
    }

    pub fn finish_base64(&self) -> String {
        encode_base64(&self.finish())
    }
}

impl Default for QuickXorHash {
    fn default() -> Self {
        Self::new()
    }
}

fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut index = 0;
    while index + 3 <= bytes.len() {
        let chunk = ((bytes[index] as u32) << 16)
            | ((bytes[index + 1] as u32) << 8)
            | (bytes[index + 2] as u32);
        out.push(TABLE[((chunk >> 18) & 0x3f) as usize] as char);
        out.push(TABLE[((chunk >> 12) & 0x3f) as usize] as char);
        out.push(TABLE[((chunk >> 6) & 0x3f) as usize] as char);
        out.push(TABLE[(chunk & 0x3f) as usize] as char);
        index += 3;
    }
    let rest = bytes.len() - index;
    if rest == 1 {
        let chunk = (bytes[index] as u32) << 16;
        out.push(TABLE[((chunk >> 18) & 0x3f) as usize] as char);
        out.push(TABLE[((chunk >> 12) & 0x3f) as usize] as char);
        out.push('=');
        out.push('=');
    } else if rest == 2 {
        let chunk = ((bytes[index] as u32) << 16) | ((bytes[index + 1] as u32) << 8);
        out.push(TABLE[((chunk >> 18) & 0x3f) as usize] as char);
        out.push(TABLE[((chunk >> 12) & 0x3f) as usize] as char);
        out.push(TABLE[((chunk >> 6) & 0x3f) as usize] as char);
        out.push('=');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_splits_match_a_single_update() {
        let sample = b"The quick brown fox jumps over the lazy dog. 2026-09-16/DJI_001.MOV";
        let mut whole = QuickXorHash::new();
        whole.update(sample);

        let mut split = QuickXorHash::new();
        for byte in sample {
            split.update(std::slice::from_ref(byte));
        }

        let mut blocks = QuickXorHash::new();
        for chunk in sample.chunks(7) {
            blocks.update(chunk);
        }

        assert_eq!(whole.finish(), split.finish());
        assert_eq!(whole.finish(), blocks.finish());
        assert_eq!(whole.finish_base64(), split.finish_base64());
    }
}
