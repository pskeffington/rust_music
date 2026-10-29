//! Standalone incremental RFC 1321 MD5 for FLAC STREAMINFO integrity checks.
//!
//! This is not an authentication primitive. No third-party hashing crate is
//! linked; it is only used to detect mismatch with an encoder-supplied digest.

#[derive(Clone)]
pub(crate) struct Md5 {
    state: [u32; 4],
    pending: [u8; 64],
    used: usize,
    total_bytes: u64,
}

const SHIFTS: [u32; 64] = [
    7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9,
    14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10, 15,
    21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
];
const TABLE: [u32; 64] = [
    0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501,
    0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
    0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
    0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed, 0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
    0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70,
    0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
    0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
    0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
];

impl Md5 {
    pub(crate) fn new() -> Self {
        Self {
            state: [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476],
            pending: [0; 64],
            used: 0,
            total_bytes: 0,
        }
    }

    fn transform(&mut self, block: &[u8; 64]) {
        let mut words = [0u32; 16];
        for (index, word) in words.iter_mut().enumerate() {
            let offset = index * 4;
            *word = u32::from_le_bytes(block[offset..offset + 4].try_into().unwrap());
        }
        let [mut a, mut b, mut c, mut d] = self.state;
        for index in 0..64 {
            let (f, g) = match index {
                0..=15 => ((b & c) | (!b & d), index),
                16..=31 => ((d & b) | (!d & c), (5 * index + 1) % 16),
                32..=47 => (b ^ c ^ d, (3 * index + 5) % 16),
                _ => (c ^ (b | !d), (7 * index) % 16),
            };
            let rotated = a
                .wrapping_add(f)
                .wrapping_add(TABLE[index])
                .wrapping_add(words[g])
                .rotate_left(SHIFTS[index]);
            a = d;
            d = c;
            c = b;
            b = b.wrapping_add(rotated);
        }
        for (state, work) in self.state.iter_mut().zip([a, b, c, d]) {
            *state = state.wrapping_add(work);
        }
    }

    pub(crate) fn update(&mut self, mut data: &[u8]) {
        self.total_bytes = self.total_bytes.wrapping_add(data.len() as u64);
        if self.used != 0 {
            let take = (64 - self.used).min(data.len());
            self.pending[self.used..self.used + take].copy_from_slice(&data[..take]);
            self.used += take;
            data = &data[take..];
            if self.used == 64 {
                let block = self.pending;
                self.transform(&block);
                self.used = 0;
            }
        }
        while data.len() >= 64 {
            let block: &[u8; 64] = data[..64].try_into().unwrap();
            self.transform(block);
            data = &data[64..];
        }
        if !data.is_empty() {
            self.pending[..data.len()].copy_from_slice(data);
            self.used = data.len();
        }
    }

    pub(crate) fn finish(mut self) -> [u8; 16] {
        let message_bits = self.total_bytes.wrapping_mul(8);
        let mut pad = [0u8; 64];
        pad[0] = 0x80;
        let pad_len = if self.used < 56 {
            56 - self.used
        } else {
            120 - self.used
        };
        self.update(&pad[..pad_len]);
        self.update(&message_bits.to_le_bytes());
        debug_assert_eq!(self.used, 0);
        let mut digest = [0u8; 16];
        for (word, bytes) in self.state.iter().zip(digest.chunks_exact_mut(4)) {
            bytes.copy_from_slice(&word.to_le_bytes());
        }
        digest
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(digest: [u8; 16]) -> String {
        digest.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    #[test]
    fn rfc_1321_digest_examples() {
        for (input, expected) in [
            ("", "d41d8cd98f00b204e9800998ecf8427e"),
            ("a", "0cc175b9c0f1b6a831c399e269772661"),
            ("abc", "900150983cd24fb0d6963f7d28e17f72"),
            ("message digest", "f96b697d7cb7938d525a2f31aaf161d0"),
            (
                "abcdefghijklmnopqrstuvwxyz",
                "c3fcd3d76192e4007dfb496cca67e13b",
            ),
        ] {
            let mut hash = Md5::new();
            for part in input.as_bytes().chunks(2) {
                hash.update(part);
            }
            assert_eq!(hex(hash.finish()), expected);
        }
    }

    #[test]
    fn chunking_does_not_change_checksum() {
        let input: Vec<u8> = (0..250_000).map(|i| (i * 41 % 251) as u8).collect();
        let mut a = Md5::new();
        a.update(&input);
        let mut b = Md5::new();
        for part in input.chunks(57) {
            b.update(part);
        }
        assert_eq!(a.finish(), b.finish());
    }
}
