// Copyright 2012-2018 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! An implementation of SipHash.
//!
//! Shamelessly stolen from the Rust std lib (libcore/hash/sip.rs) and adapted.
//!
//! Only the official variant, SipHash 2-4, is included. Other variants such
//! as the reduced 1-3 (used by libstd's `DefaultHasher`) could be added easily.

use core::marker::PhantomData;
use core::{cmp, hash, mem, ptr, slice};
use rand_core::{impls, le, Error, RngCore, SeedableRng};

/// A portable implementation of SipHash 2-4.
///
/// This implementation will produce 8-byte (`u64`) output compliant with the
/// reference implementation. Additionally, it can be extended into an RNG able
/// to produce unlimited output via [`SipHasher::into_rng`].
///
/// See: <https://131002.net/siphash/>
///
/// SipHash is a general-purpose hashing function: it runs at a good
/// speed (competitive with Spooky and City) and permits strong keyed hashing.
///
/// Although the SipHash algorithm is considered strong, it is not intended for
/// cryptographic uses (e.g. authentication).
#[derive(Debug, Clone, Default)]
pub struct SipHasher {
    hasher: Hasher<Sip24Rounds>,
}

/// A generator built using SipHash's primitives.
///
/// [`SipRng`] is statistically high-quality, passing practrand tests to at
/// least 4 TiB. It is also reasonably fast, though not quite competitive with
/// the best non-cryptographic RNGs or optimised block RNGs such as ChaCha.
///
/// This implementation is fixed to use two "compression" rounds between output
/// values (similar to SipHash 2-4). Construction via [`SipHasher::into_rng`]
/// adds two extra rounds to maintain four rounds between final input
/// consumption and the first output, however this first result is not identical
/// to SipHash's result.
///
/// Although this generator is heavily based on the design of SipHash, it has
/// not been reviewed for cryptographic strength, and thus cannot be recommended
/// for applications requiring this property.
#[derive(Debug, Clone)]
pub struct SipRng {
    state: State,
    adj: u64,
}

#[derive(Debug)]
struct Hasher<S: Sip> {
    k0: u64,
    k1: u64,
    length: usize, // how many bytes we've processed
    state: State,  // hash State
    tail: u64,     // unprocessed bytes le
    ntail: usize,  // how many bytes in tail are valid
    _marker: PhantomData<S>,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct State {
    // v0, v2 and v1, v3 show up in pairs in the algorithm,
    // and simd implementations of SipHash will use vectors
    // of v02 and v13. By placing them in this order in the struct,
    // the compiler can pick up on just a few simd optimizations by itself.
    v0: u64,
    v2: u64,
    v1: u64,
    v3: u64,
}

macro_rules! compress {
    ($state:expr) => {{
        compress!($state.v0, $state.v1, $state.v2, $state.v3)
    }};
    ($v0:expr, $v1:expr, $v2:expr, $v3:expr) => {{
        $v0 = $v0.wrapping_add($v1);
        $v1 = $v1.rotate_left(13);
        $v1 ^= $v0;
        $v0 = $v0.rotate_left(32);
        $v2 = $v2.wrapping_add($v3);
        $v3 = $v3.rotate_left(16);
        $v3 ^= $v2;
        $v0 = $v0.wrapping_add($v3);
        $v3 = $v3.rotate_left(21);
        $v3 ^= $v0;
        $v2 = $v2.wrapping_add($v1);
        $v1 = $v1.rotate_left(17);
        $v1 ^= $v2;
        $v2 = $v2.rotate_left(32);
    }};
}

/// Loads an integer of the desired type from a byte stream, in LE order. Uses
/// `copy_nonoverlapping` to let the compiler generate the most efficient way
/// to load it from a possibly unaligned address.
///
/// Unsafe because: unchecked indexing at i..i+size_of(int_ty)
macro_rules! load_int_le {
    ($buf:expr, $i:expr, $int_ty:ident) => {{
        debug_assert!($i + mem::size_of::<$int_ty>() <= $buf.len());
        let mut data = 0 as $int_ty;
        ptr::copy_nonoverlapping(
            $buf.as_ptr().add($i),
            &mut data as *mut _ as *mut u8,
            mem::size_of::<$int_ty>(),
        );
        data.to_le()
    }};
}

/// Loads an u64 using up to 7 bytes of a byte slice.
///
/// Unsafe because: unchecked indexing at start..start+len
#[inline]
unsafe fn u8to64_le(buf: &[u8], start: usize, len: usize) -> u64 {
    debug_assert!(len < 8);
    let mut i = 0; // current byte index (from LSB) in the output u64
    let mut out = 0;
    if i + 3 < len {
        out = load_int_le!(buf, start + i, u32) as u64;
        i += 4;
    }
    if i + 1 < len {
        out |= (load_int_le!(buf, start + i, u16) as u64) << (i * 8);
        i += 2
    }
    if i < len {
        out |= (*buf.as_ptr().add(start + i) as u64) << (i * 8);
        i += 1;
    }
    debug_assert_eq!(i, len);
    out
}

impl SipHasher {
    /// Create a new `SipHasher` (equivalent to `from_keys(0, 0)`).
    #[inline]
    pub fn new() -> Self {
        SipHasher::from_keys(0, 0)
    }

    /// Create a `SipHasher` using the given keys.
    #[inline]
    pub fn from_keys(key0: u64, key1: u64) -> Self {
        SipHasher {
            hasher: Hasher::from_keys(key0, key1),
        }
    }

    /// Finish writes and convert the hasher's core into a generator.
    ///
    /// This offers a fast, elegant transition from hash function to generator
    /// which maintains (up to) 256 bits of entropy.
    ///
    /// Note that this transition has not been reviewed for cryptographic
    /// strength, and might break SipHash's security.
    pub fn into_rng(self) -> SipRng {
        self.hasher.into_rng()
    }
}

impl SipRng {
    // private constructor
    fn from_state(state: State) -> SipRng {
        SipRng { state, adj: 0x13 }
    }
}

impl RngCore for SipRng {
    fn next_u32(&mut self) -> u32 {
        // Lazy way to implement. Good enough for seeding RNGs.
        self.next_u64() as u32
    }

    fn next_u64(&mut self) -> u64 {
        self.state.v2 ^= self.adj;
        self.adj = self.adj.wrapping_sub(0x11);

        Sip24Rounds::c_rounds(&mut self.state);

        self.state.v0 ^ self.state.v1 ^ self.state.v2 ^ self.state.v3
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        impls::fill_bytes_via_next(self, dest)
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Error> {
        Ok(self.fill_bytes(dest))
    }
}

impl SeedableRng for SipRng {
    type Seed = [u8; 32];

    fn from_seed(seed: Self::Seed) -> Self {
        let mut keys = [0u64; 4];
        le::read_u64_into(&seed, &mut keys);
        SipRng::from_state(State {
            v0: keys[0],
            v1: keys[1],
            v2: keys[2],
            v3: keys[3],
        })
    }
}

impl<S: Sip> Hasher<S> {
    #[inline]
    fn from_keys(key0: u64, key1: u64) -> Hasher<S> {
        let mut state = Hasher {
            k0: key0,
            k1: key1,
            length: 0,
            state: State {
                v0: 0,
                v1: 0,
                v2: 0,
                v3: 0,
            },
            tail: 0,
            ntail: 0,
            _marker: PhantomData,
        };
        state.reset();
        state
    }

    #[inline]
    fn reset(&mut self) {
        self.length = 0;
        self.state.v0 = self.k0 ^ 0x736f6d6570736575;
        self.state.v1 = self.k1 ^ 0x646f72616e646f6d;
        self.state.v2 = self.k0 ^ 0x6c7967656e657261;
        self.state.v3 = self.k1 ^ 0x7465646279746573;
        self.ntail = 0;
    }

    // Specialized write function that is only valid for buffers with len <= 8.
    // It's used to force inlining of write_u8 and write_usize, those would normally be inlined
    // except for composite types (that includes slices and str hashing because of delimiter).
    // Without this extra push the compiler is very reluctant to inline delimiter writes,
    // degrading performance substantially for the most common use cases.
    #[inline]
    fn short_write(&mut self, msg: &[u8]) {
        debug_assert!(msg.len() <= 8);
        let length = msg.len();
        self.length += length;

        let needed = 8 - self.ntail;
        let fill = cmp::min(length, needed);
        if fill == 8 {
            self.tail = unsafe { load_int_le!(msg, 0, u64) };
        } else {
            self.tail |= unsafe { u8to64_le(msg, 0, fill) } << (8 * self.ntail);
            if length < needed {
                self.ntail += length;
                return;
            }
        }
        self.state.v3 ^= self.tail;
        S::c_rounds(&mut self.state);
        self.state.v0 ^= self.tail;

        // Buffered tail is now flushed, process new input.
        self.ntail = length - needed;
        self.tail = unsafe { u8to64_le(msg, needed, self.ntail) };
    }

    fn into_rng(self) -> SipRng {
        let mut state = self.state;

        // Finish
        let b: u64 = ((self.length as u64 & 0xff) << 56) | self.tail;

        state.v3 ^= b;
        S::c_rounds(&mut state);
        state.v0 ^= b;

        // This is supposed to be d - c rounds (here 4 - 2 = 2)
        S::c_rounds(&mut state);

        SipRng::from_state(state)
    }
}

/// Implements the standard library's `Hasher` trait.
///
/// Note that all methods are implemented directly to fix Endianness, unlike
/// the default implementations in the standard library.
impl hash::Hasher for SipHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.hasher.finish()
    }

    #[inline]
    fn write(&mut self, msg: &[u8]) {
        self.hasher.write(msg)
    }

    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.write(&[i])
    }
    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.write(&i.to_le_bytes())
    }
    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.write(&i.to_le_bytes())
    }
    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.write(&i.to_le_bytes())
    }
    #[inline]
    fn write_u128(&mut self, i: u128) {
        self.write(&i.to_le_bytes())
    }

    /// For portability reasons, the `usize` input is interpreted as a `u128`.
    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.write_u128(i as u128);
    }

    #[inline]
    fn write_i8(&mut self, i: i8) {
        self.write_u8(i as u8)
    }
    #[inline]
    fn write_i16(&mut self, i: i16) {
        self.write(&i.to_le_bytes())
    }
    #[inline]
    fn write_i32(&mut self, i: i32) {
        self.write(&i.to_le_bytes())
    }
    #[inline]
    fn write_i64(&mut self, i: i64) {
        self.write(&i.to_le_bytes())
    }
    #[inline]
    fn write_i128(&mut self, i: i128) {
        self.write(&i.to_le_bytes())
    }

    /// For portability reasons, the `isize` input is interpreted as a `i128`.
    #[inline]
    fn write_isize(&mut self, i: isize) {
        self.write_i128(i as i128);
    }
}

impl<S: Sip> hash::Hasher for Hasher<S> {
    // see short_write comment for explanation
    #[inline]
    fn write_usize(&mut self, i: usize) {
        let bytes = unsafe {
            slice::from_raw_parts(&i as *const usize as *const u8, mem::size_of::<usize>())
        };
        self.short_write(bytes);
    }

    // see short_write comment for explanation
    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.short_write(&[i]);
    }

    #[inline]
    fn write(&mut self, msg: &[u8]) {
        let length = msg.len();
        self.length += length;

        let mut needed = 0;

        if self.ntail != 0 {
            needed = 8 - self.ntail;
            self.tail |= unsafe { u8to64_le(msg, 0, cmp::min(length, needed)) } << 8 * self.ntail;
            if length < needed {
                self.ntail += length;
                return;
            } else {
                self.state.v3 ^= self.tail;
                S::c_rounds(&mut self.state);
                self.state.v0 ^= self.tail;
                self.ntail = 0;
            }
        }

        // Buffered tail is now flushed, process new input.
        let len = length - needed;
        let left = len & 0x7;

        let mut i = needed;
        while i < len - left {
            let mi = unsafe { load_int_le!(msg, i, u64) };

            self.state.v3 ^= mi;
            S::c_rounds(&mut self.state);
            self.state.v0 ^= mi;

            i += 8;
        }

        self.tail = unsafe { u8to64_le(msg, i, left) };
        self.ntail = left;
    }

    /// Reduces the state to a `u64` value, as in the reference implementation.
    #[inline]
    fn finish(&self) -> u64 {
        let mut state = self.state;

        let b: u64 = ((self.length as u64 & 0xff) << 56) | self.tail;

        state.v3 ^= b;
        S::c_rounds(&mut state);
        state.v0 ^= b;

        state.v2 ^= 0xff;
        S::d_rounds(&mut state);

        state.v0 ^ state.v1 ^ state.v2 ^ state.v3
    }
}

impl<H: hash::Hash> From<H> for SipHasher {
    #[inline]
    fn from(h: H) -> SipHasher {
        let mut hasher = SipHasher::new();
        h.hash(&mut hasher);
        hasher
    }
}

impl<S: Sip> Clone for Hasher<S> {
    #[inline]
    fn clone(&self) -> Hasher<S> {
        Hasher {
            k0: self.k0,
            k1: self.k1,
            length: self.length,
            state: self.state,
            tail: self.tail,
            ntail: self.ntail,
            _marker: self._marker,
        }
    }
}

impl<S: Sip> Default for Hasher<S> {
    /// Creates a `Hasher<S>` with the two initial keys set to 0.
    #[inline]
    fn default() -> Hasher<S> {
        Hasher::from_keys(0, 0)
    }
}

#[doc(hidden)]
trait Sip {
    fn c_rounds(_: &mut State);
    fn d_rounds(_: &mut State);
}

#[derive(Debug, Clone, Default)]
struct Sip24Rounds;

impl Sip for Sip24Rounds {
    #[inline]
    fn c_rounds(state: &mut State) {
        compress!(state);
        compress!(state);
    }

    #[inline]
    fn d_rounds(state: &mut State) {
        compress!(state);
        compress!(state);
        compress!(state);
        compress!(state);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use core::hash::Hasher;

    #[test]
    fn test_hash_vectors() {
        let k_bytes = [0u8, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
        let (k0, k1) = unsafe {
            (
                load_int_le!(&k_bytes, 0, u64),
                load_int_le!(&k_bytes, 8, u64),
            )
        };

        let mut msg = [0u8; 64];
        for (pos, i) in msg.iter_mut().zip(0u8..64) {
            *pos = i;
        }

        // From reference vectors, converted to u64:
        let tests = [
            (0, 0x726fdb47dd0e0e31),
            (1, 0x74f839c593dc67fd),
            (63, 0x958a324ceb064572),
        ];

        for (len, val) in &tests {
            let mut hasher = SipHasher::from_keys(k0, k1);
            hasher.write(&msg[0..*len]);
            assert_eq!(
                hasher.finish(),
                *val,
                "mismatch with reference vector for i={}",
                *len
            );
        }
    }

    #[test]
    fn test_rng_vectors() {
        // SipRng has no external reference. These vectors are defined here.

        let mut seed = [0u8; 32];
        let mut rng = SipRng::from_seed(seed);

        let vector = [
            0x4c022e4ec04e602a,
            0xc2c0399c269058d6,
            0xf5c7399cde9c362c,
            0x37e5b9491363680a,
            0x9582782644903316,
            0x2a9d2e160aad88d,
            0x983958db9376e6f6,
            0xdead8960b8524928,
            0xcfa886c6642c1b2f,
            0x8f8f91fcf7045f2a,
            0x1bbda585fc387fb3,
            0x242485d9cc54c688,
            0x9be110f767d8cee,
            0xd61076dfc3569ab3,
            0x8f6092dd2692af57,
            0xbdf362ab8e29260b,
        ];
        // for _ in 0..8 {
        //     println!("0x{:x}, 0x{:x},", rng.next_u64(), rng.next_u64());
        // }
        for i in 0..16 {
            assert_eq!(rng.next_u64(), vector[i]);
        }

        // set seed to 0, 1, 2, ..., 31
        for (pos, i) in seed.iter_mut().zip(0u8..32) {
            *pos = i;
        }
        let mut rng = SipRng::from_seed(seed);

        let vector = [
            0x479bf2823a7a923e,
            0xf04e2cbc75d554d,
            0xd589aceb3b65f36b,
            0x91f8758ab30951a,
            0x10d2bebadd90c381,
            0xb3a6345b6273b101,
            0xd05dbd603684e153,
            0xabaaa983f818f5db,
            0x2a063ed10d464bf2,
            0x1d395c4c511e9073,
            0x43011ca87ead4d7c,
            0x22acb2bfbca6a069,
            0xdd6b8dd2abb4d8f,
            0xb3bc3889e7142461,
            0x62cbac703609d15,
            0x74aec28d9fdd44bf,
        ];
        for i in 0..16 {
            assert_eq!(rng.next_u64(), vector[i]);
        }
    }
}
