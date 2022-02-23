// Copyright 2018 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// https://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A universal seeder based on [SipHash].
//!
//! This crate is designed for use with the [rand] crates, allowing any RNG
//! supporting [`SeedableRng`] to be seeded from any hashable value.
//! It provides the following:
//!
//! -   [`SipHasher`], a portable implementation of the SipHash 2-4 hash function.
//!     According to the authors, [SipHash] is a secure, fast and simple keyed
//!     hash function.
//! -   [`SipRng`], a generator based on the SipHash state and mixing operations.
//!     It is statistically high-quality, passing practrand tests to at least 4 TiB.
//! -   A universal [`Seeder`] as a convenience wrapper around the above.
//!
//! Seeding is designed to be fast, robust, flexible and portable. This library
//! is intended for use in simulations and games, allowing e.g. any keyword to
//! reproduce a simulation or procedurally generated world.
//!
//! This library is not intended for cryptographic applications, and *definitely*
//! not for password hashing.
//!
//! [rand]: https://github.com/rust-random/rand
//! [SipHash]: https://131002.net/siphash/
//! [`SeedableRng`]: rand_core::SeedableRng

#![doc(
    html_logo_url = "https://www.rust-lang.org/logos/rust-logo-128x128-blk.png",
    html_favicon_url = "https://www.rust-lang.org/favicon.ico",
    html_root_url = "https://docs.rs/rand_seeder/0.2.3"
)]
#![deny(missing_docs)]
#![deny(missing_debug_implementations)]
#![doc(test(attr(allow(unused_variables), deny(warnings))))]
#![no_std]

pub extern crate rand_core;

mod sip;

pub use sip::{SipHasher, SipRng};

use core::hash::Hash;
use rand_core::{RngCore, SeedableRng};

/// A universal seeder.
///
/// `Seeder` can be used to seed any [`SeedableRng`] from any hashable value. It
/// is portable and reproducible, and should turn any input into a good RNG
/// seed. It is intended for use in simulations and games where reproducibility
/// is important.
///
/// We do not recommend using `Seeder` for cryptographic applications and
/// strongly advise against usage for authentication (password hashing).
///
/// Example:
///
/// ```rust
/// # extern crate rand_core;
/// # extern crate rand_seeder;
/// use rand_core::RngCore;
/// use rand_seeder::{Seeder, SipRng};
///
/// // Use any SeedableRng you like in place of SipRng:
/// let mut rng: SipRng = Seeder::from("stripy zebra").make_rng();
/// println!("First value: {}", rng.next_u32());
/// ```
///
/// [`SeedableRng`]: rand_core::SeedableRng
#[derive(Debug, Clone)]
pub struct Seeder {
    rng: SipRng,
}

impl Seeder {
    /// Construct and seed a generator of any type `R: SeedableRng`.
    ///
    /// This function seeds a new RNG using an internal [`SipRng`] generator.
    /// Because of this, multiple independent RNGs may safely be constructed
    /// from a single [`Seeder`].
    ///
    /// Alternatively, one can obtain a [`SipRng`] via
    /// `SipHasher::from(h).into_rng()`.
    #[inline]
    pub fn make_rng<R: SeedableRng>(&mut self) -> R {
        R::from_seed(self.make_seed())
    }

    /// Make a seed
    ///
    /// This mutates the state internally, thus can be called multiple times to
    /// generate multiple independent seeds.
    #[inline]
    pub fn make_seed<S: AsMut<[u8]> + Default>(&mut self) -> S {
        let mut seed = S::default();
        self.rng.fill_bytes(seed.as_mut());
        seed
    }
}

impl<H: Hash> From<H> for Seeder {
    #[inline]
    fn from(h: H) -> Seeder {
        let hasher = SipHasher::from(h);
        Seeder {
            rng: hasher.into_rng(),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn make_seeder() {
        let _ = Seeder::from(0u64);
        let _ = Seeder::from("a static string");
        let _ = Seeder::from([1u8, 2, 3]);
    }

    #[test]
    fn make_rng() {
        let mut seeder = Seeder::from("test string");
        let mut rng = seeder.make_rng::<SipRng>();
        assert_eq!(rng.next_u64(), 7267854722795183454);
        assert_eq!(rng.next_u64(), 602994585684902144);
    }
}
