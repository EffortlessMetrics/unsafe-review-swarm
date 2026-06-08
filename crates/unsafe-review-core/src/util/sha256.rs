//! Hand-rolled SHA-256 implementation in safe Rust.
//!
//! This exists to bind provenance artifacts to their input content without adding
//! a dependency. SHA-256 is chosen because it is collision-resistant; the
//! existing FNV-64 hash is NOT used here because 64-bit FNV is not
//! collision-resistant and this field exists to bind an artifact to its input.
//!
//! Algorithm: FIPS 180-4 (SHA-256). Constants verified against NIST test vectors.
//!
//! Trust boundary: this is a content-binding digest, not a cryptographic security
//! primitive; it establishes traceable evidence metadata, not proof of integrity.

/// Initial hash values (H0): first 32 bits of fractional parts of
/// square roots of the first 8 primes (2, 3, 5, 7, 11, 13, 17, 19).
/// Source: FIPS 180-4, Section 5.3.3.
const INIT: [u32; 8] = [
    0x6a09_e667,
    0xbb67_ae85,
    0x3c6e_f372,
    0xa54f_f53a,
    0x510e_527f,
    0x9b05_688c,
    0x1f83_d9ab,
    0x5be0_cd19,
];

/// Round constants (K): first 32 bits of fractional parts of
/// cube roots of the first 64 primes.
/// Source: FIPS 180-4, Section 4.2.2.
const K: [u32; 64] = [
    0x428a_2f98,
    0x7137_4491,
    0xb5c0_fbcf,
    0xe9b5_dba5,
    0x3956_c25b,
    0x59f1_11f1,
    0x923f_82a4,
    0xab1c_5ed5,
    0xd807_aa98,
    0x1283_5b01,
    0x2431_85be,
    0x550c_7dc3,
    0x72be_5d74,
    0x80de_b1fe,
    0x9bdc_06a7,
    0xc19b_f174,
    0xe49b_69c1,
    0xefbe_4786,
    0x0fc1_9dc6,
    0x240c_a1cc,
    0x2de9_2c6f,
    0x4a74_84aa,
    0x5cb0_a9dc,
    0x76f9_88da,
    0x983e_5152,
    0xa831_c66d,
    0xb003_27c8,
    0xbf59_7fc7,
    0xc6e0_0bf3,
    0xd5a7_9147,
    0x06ca_6351,
    0x1429_2967,
    0x27b7_0a85,
    0x2e1b_2138,
    0x4d2c_6dfc,
    0x5338_0d13,
    0x650a_7354,
    0x766a_0abb,
    0x81c2_c92e,
    0x9272_2c85,
    0xa2bf_e8a1,
    0xa81a_664b,
    0xc24b_8b70,
    0xc76c_51a3,
    0xd192_e819,
    0xd699_0624,
    0xf40e_3585,
    0x106a_a070,
    0x19a4_c116,
    0x1e37_6c08,
    0x2748_774c,
    0x34b0_bcb5,
    0x391c_0cb3,
    0x4ed8_aa4a,
    0x5b9c_ca4f,
    0x682e_6ff3,
    0x748f_82ee,
    0x78a5_636f,
    0x84c8_7814,
    0x8cc7_0208,
    0x90be_fffa,
    0xa450_6ceb,
    0xbef9_a3f7,
    0xc671_78f2,
];

/// Compute SHA-256 of `data` and return the hex-encoded digest (64 lowercase hex chars).
pub(crate) fn sha256_hex(data: &[u8]) -> String {
    let digest = sha256(data);
    let mut out = String::with_capacity(64);
    for byte in &digest {
        use std::fmt::Write as _;
        let _ok = write!(out, "{byte:02x}");
    }
    out
}

/// Compute the raw 32-byte SHA-256 digest.
fn sha256(data: &[u8]) -> [u8; 32] {
    let mut state = INIT;
    let bit_len: u64 = (data.len() as u64).wrapping_mul(8);

    // Padding: append 0x80, zeros, then the 64-bit big-endian bit length.
    // The padded message length is ≡ 0 (mod 64).
    let mut padded: Vec<u8> = Vec::with_capacity(data.len() + 1 + 8 + 64);
    padded.extend_from_slice(data);
    padded.push(0x80);
    // Pad with zeros until length ≡ 56 (mod 64).
    while padded.len() % 64 != 56 {
        padded.push(0x00);
    }
    // Append 64-bit big-endian bit length.
    padded.extend_from_slice(&bit_len.to_be_bytes());

    // Process each 64-byte block.
    for block in padded.chunks_exact(64) {
        compress(&mut state, block);
    }

    // Produce final digest: concatenate state words as big-endian bytes.
    let mut out = [0u8; 32];
    for (i, &word) in state.iter().enumerate() {
        out[i * 4..(i + 1) * 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

/// Compress one 64-byte block into the running state.
fn compress(state: &mut [u32; 8], block: &[u8]) {
    debug_assert_eq!(block.len(), 64);

    // Build message schedule W[0..64].
    let mut w = [0u32; 64];
    for (i, chunk) in block.chunks_exact(4).enumerate().take(16) {
        w[i] = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
    }
    for i in 16..64 {
        // σ0(w[i-15])
        let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
        // σ1(w[i-2])
        let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16]
            .wrapping_add(s0)
            .wrapping_add(w[i - 7])
            .wrapping_add(s1);
    }

    // Initialize working variables.
    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;

    // 64 rounds.
    for i in 0..64 {
        // Σ1(e)
        let sum1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        // Ch(e, f, g)
        let ch = (e & f) ^ (!e & g);
        let t1 = h
            .wrapping_add(sum1)
            .wrapping_add(ch)
            .wrapping_add(K[i])
            .wrapping_add(w[i]);
        // Σ0(a)
        let sum0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        // Maj(a, b, c)
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let t2 = sum0.wrapping_add(maj);

        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(t1);
        d = c;
        c = b;
        b = a;
        a = t1.wrapping_add(t2);
    }

    // Add compressed chunk to current hash value.
    state[0] = state[0].wrapping_add(a);
    state[1] = state[1].wrapping_add(b);
    state[2] = state[2].wrapping_add(c);
    state[3] = state[3].wrapping_add(d);
    state[4] = state[4].wrapping_add(e);
    state[5] = state[5].wrapping_add(f);
    state[6] = state[6].wrapping_add(g);
    state[7] = state[7].wrapping_add(h);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// NIST vector: SHA-256("") =
    /// e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
    #[test]
    fn sha256_empty_string_nist_vector() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    /// NIST vector: SHA-256("abc") =
    /// ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
    ///
    /// Verified with sha256sum, Python hashlib, and Windows CNG on this platform.
    #[test]
    fn sha256_abc_nist_vector() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    /// NIST vector (multi-block, >64 bytes):
    /// SHA-256("abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq") =
    /// 248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1
    #[test]
    fn sha256_multi_block_nist_vector() {
        assert_eq!(
            sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    /// Sanity: two distinct inputs produce distinct digests.
    #[test]
    fn sha256_distinct_inputs_produce_distinct_digests() {
        assert_ne!(sha256_hex(b"hello"), sha256_hex(b"world"));
        assert_ne!(sha256_hex(b"hello"), sha256_hex(b"hello\n"));
    }

    /// Output is always 64 lowercase hex characters.
    #[test]
    fn sha256_output_is_64_lowercase_hex_chars() {
        for input in [b"".as_slice(), b"x", b"unsafe-review"] {
            let hex = sha256_hex(input);
            assert_eq!(hex.len(), 64, "input={input:?}");
            assert!(
                hex.chars()
                    .all(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase()),
                "sha256_hex({input:?}) produced non-lowercase-hex output: {hex}"
            );
        }
    }
}
