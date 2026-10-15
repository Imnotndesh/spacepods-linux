//! Bluetooth legacy **E1** (SAFER+) authentication primitive.
//!
//! JieLi's RCSP device authentication is a challenge/response where the
//! response is `E1(K, challenge, addr)` using two hardcoded secrets. This was
//! reverse-engineered by Quarkslab:
//! <https://blog.quarkslab.com/modern-tale-blinkenlights.html>
//!
//! The same key and identity address are present in both
//! `libjl_ota_auth.so` and `libjl_bluetooth.so` shipped in the Oraimo Sound
//! APK, so the algorithm is reused here.
//!
//! Ported from the BIAS reference implementation (`la/h.py`, francozappa/bias).

use std::sync::OnceLock;

/// JieLi's hardcoded RCSP authentication link key.
pub const JIELI_AUTH_KEY: [u8; 16] = [
    0x06, 0x77, 0x5F, 0x87, 0x91, 0x8D, 0xD4, 0x23, 0x00, 0x5D, 0xF1, 0xD8, 0xCF, 0x0C, 0x14,
    0x2B,
];

/// JieLi's hardcoded "device identity" Bluetooth address.
const JIELI_AUTH_ADDR: [u8; 6] = [0x11, 0x22, 0x33, 0x33, 0x22, 0x11];

/// Byte positions that use addition in `add_one` / `add_two` and the
/// exponentiation half of `nonlin_subs`.
const ODD_POS: [usize; 8] = [0, 3, 4, 7, 8, 11, 12, 15];

fn mod_pow(mut base: u64, mut exp: u64, modulus: u64) -> u64 {
    if modulus == 1 {
        return 0;
    }
    let mut result = 1u64;
    base %= modulus;
    while exp > 0 {
        if exp & 1 == 1 {
            result = (result * base) % modulus;
        }
        exp >>= 1;
        base = (base * base) % modulus;
    }
    result
}

/// SAFER+ exponentiation table: `45^i mod 257 mod 256`.
fn exp45() -> &'static [u8; 256] {
    static TABLE: OnceLock<[u8; 256]> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut t = [0u8; 256];
        for (i, slot) in t.iter_mut().enumerate() {
            *slot = (mod_pow(45, i as u64, 257) % 256) as u8;
        }
        t
    })
}

/// Inverse (discrete log base 45) of [`exp45`].
fn log45() -> &'static [u8; 256] {
    static TABLE: OnceLock<[u8; 256]> = OnceLock::new();
    TABLE.get_or_init(|| {
        let exp = exp45();
        let mut t = [0u8; 256];
        for (i, &v) in exp.iter().enumerate() {
            t[v as usize] = i as u8;
        }
        t
    })
}

/// SAFER+ key-schedule bias constants `B[2..18]`.
fn biases() -> &'static [[u8; 16]; 18] {
    static TABLE: OnceLock<[[u8; 16]; 18]> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut b = [[0u8; 16]; 18];
        for n in 2..18usize {
            for i in 0..16usize {
                let inner = mod_pow(45, (17 * n + i + 1) as u64, 257);
                let v = mod_pow(45, inner, 257) % 256;
                b[n][i] = v as u8;
            }
        }
        b
    })
}

/// Compute the 16-byte expected RCSP auth response for a challenge.
///
/// The challenge may be any length; only the first 16 bytes are used and it is
/// zero-padded if shorter.
pub fn jieli_encrypt(challenge: &[u8]) -> [u8; 16] {
    let mut c = [0u8; 16];
    let n = challenge.len().min(16);
    c[..n].copy_from_slice(&challenge[..n]);
    h(&JIELI_AUTH_KEY, &c, &JIELI_AUTH_ADDR, 6)
}

/// The `H` hash function used by E1. Returns `SRES || ACO` (16 bytes).
fn h(key: &[u8; 16], i_one: &[u8; 16], i_two: &[u8], l: usize) -> [u8; 16] {
    let keys = key_sched(key);
    let k_tilda = k_to_k_tilda(key);
    let keys_prime = key_sched(&k_tilda);

    let i_two_ext = e_expand(i_two, l);
    let ar = ar_rounds(&keys, i_one, false);

    let mut pre_ar_prime_inp = [0u8; 16];
    for i in 0..16 {
        pre_ar_prime_inp[i] = ar[10][i] ^ i_one[i];
    }
    let mut ar_prime_inp = [0u8; 16];
    for i in 0..16 {
        ar_prime_inp[i] = i_two_ext[i].wrapping_add(pre_ar_prime_inp[i]);
    }

    let ar_prime = ar_rounds(&keys_prime, &ar_prime_inp, true);
    ar_prime[10]
}

fn key_sched(key: &[u8; 16]) -> [[u8; 16]; 18] {
    let mut keys = [[0u8; 16]; 18];
    let mut byte16 = 0u8;
    for &k in key {
        byte16 ^= k;
    }
    let mut presel = [0u8; 17];
    presel[..16].copy_from_slice(key);
    presel[16] = byte16;
    keys[1] = select(1, &presel);

    let b = biases();
    for n in 2..18usize {
        let rotated = rotate(&presel);
        let pre = select(n, &rotated);
        for i in 0..16 {
            keys[n][i] = pre[i].wrapping_add(b[n][i]);
        }
        presel = rotated;
    }
    keys
}

/// `(EQ 13)` select 16 bytes from a 17-byte buffer.
fn select(what: usize, key: &[u8; 17]) -> [u8; 16] {
    let start = what - 1;
    let mut out = [0u8; 16];
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = key[(start + i) % 17];
    }
    out
}

fn rotate(key: &[u8; 17]) -> [u8; 17] {
    let mut out = [0u8; 17];
    for i in 0..17 {
        out[i] = key[i].rotate_left(3);
    }
    out
}

fn add_one(l: &[u8; 16], r: &[u8; 16]) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..16 {
        out[i] = if ODD_POS.contains(&i) {
            l[i] ^ r[i]
        } else {
            l[i].wrapping_add(r[i])
        };
    }
    out
}

fn add_two(l: &[u8; 16], r: &[u8; 16]) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..16 {
        out[i] = if ODD_POS.contains(&i) {
            l[i].wrapping_add(r[i])
        } else {
            l[i] ^ r[i]
        };
    }
    out
}

fn nonlin_subs(inp: &[u8; 16]) -> [u8; 16] {
    let exp = exp45();
    let log = log45();
    let mut out = [0u8; 16];
    for i in 0..16 {
        out[i] = if ODD_POS.contains(&i) {
            exp[inp[i] as usize]
        } else {
            log[inp[i] as usize]
        };
    }
    out
}

fn pht(x: u8, y: u8) -> (u8, u8) {
    let rv_x = (2u16 * x as u16 + y as u16) as u8;
    let rv_y = (x as u16 + y as u16) as u8;
    (rv_x, rv_y)
}

fn phts(inp: &[u8; 16]) -> [u8; 16] {
    let mut out = [0u8; 16];
    let mut i = 0;
    while i < 16 {
        let (x, y) = pht(inp[i], inp[i + 1]);
        out[i] = x;
        out[i + 1] = y;
        i += 2;
    }
    out
}

/// The Armenian permutation.
fn permute(inp: &[u8; 16]) -> [u8; 16] {
    const ORDER: [usize; 16] = [8, 11, 12, 15, 2, 1, 6, 5, 10, 9, 14, 13, 0, 7, 4, 3];
    let mut out = [0u8; 16];
    for (i, &src) in ORDER.iter().enumerate() {
        out[i] = inp[src];
    }
    out
}

/// Run the 8 `Ar` rounds. `out[0]` is unused; `out[1]` is the input;
/// `out[10]` is the final keyed addition.
fn ar_rounds(keys: &[[u8; 16]; 18], inp: &[u8; 16], is_prime: bool) -> [[u8; 16]; 11] {
    let mut ar = [[0u8; 16]; 11];
    ar[1] = *inp;
    let mut temp = *inp;

    for r in 1..=8usize {
        if is_prime && r == 3 {
            temp = add_one(&temp, &ar[1]);
        }
        let rv1 = add_one(&temp, &keys[2 * r - 1]);
        let rv2 = nonlin_subs(&rv1);
        let rv3 = add_two(&rv2, &keys[2 * r]);
        let rv4 = phts(&rv3);
        let rv5 = permute(&rv4);
        let rv6 = phts(&rv5);
        let rv7 = permute(&rv6);
        let rv8 = phts(&rv7);
        let rv9 = permute(&rv8);
        let rv10 = phts(&rv9);
        temp = rv10;
        ar[r + 1] = rv10;
    }

    ar[10] = add_one(&ar[9], &keys[17]);
    ar
}

fn k_to_k_tilda(k: &[u8; 16]) -> [u8; 16] {
    let mut out = [0u8; 16];
    out[0] = k[0].wrapping_add(233);
    out[1] = k[1] ^ 229;
    out[2] = k[2].wrapping_add(223);
    out[3] = k[3] ^ 193;
    out[4] = k[4].wrapping_add(179);
    out[5] = k[5] ^ 167;
    out[6] = k[6].wrapping_add(149);
    out[7] = k[7] ^ 131;
    out[8] = k[8] ^ 233;
    out[9] = k[9].wrapping_add(229);
    out[10] = k[10] ^ 223;
    out[11] = k[11].wrapping_add(193);
    out[12] = k[12] ^ 179;
    out[13] = k[13].wrapping_add(167);
    out[14] = k[14] ^ 149;
    out[15] = k[15].wrapping_add(131);
    out
}

/// `(EQ 14)` expand an `l`-byte input to 16 bytes by repetition.
fn e_expand(inp: &[u8], l: usize) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..16 {
        out[i] = inp[i % l];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unhex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    /// Test vector from the Quarkslab write-up.
    #[test]
    fn test_jieli_auth_vector() {
        let challenge = unhex("ABB2CDC69BB454110E827441213DDC87");
        let response = jieli_encrypt(&challenge);
        assert_eq!(
            response.to_vec(),
            unhex("61721F97C869523CCCE093D7F849CA02")
        );
    }

    /// Standard SAFER+ / E1 sanity: deterministic and 16 bytes.
    #[test]
    fn test_deterministic() {
        let a = jieli_encrypt(&[0u8; 16]);
        let b = jieli_encrypt(&[0u8; 16]);
        assert_eq!(a, b);
        assert_eq!(a.len(), 16);
    }
}
