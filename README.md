# rand_seeder

[![Build Status](https://travis-ci.org/rust-random/seeder.svg)](https://travis-ci.org/rust-random/seeder)
[![Latest version](https://img.shields.io/crates/v/rand_seeder.svg)](https://crates.io/crates/rand_seeder)
[![Documentation](https://docs.rs/rand_seeder/badge.svg)](https://docs.rs/rand_seeder)
[![Minimum rustc version](https://img.shields.io/badge/rustc-1.32+-yellow.svg)](https://github.com/rust-random/seeder#rust-version-requirements)
[![License](https://img.shields.io/crates/l/rand_seeder.svg)](https://github.com/rust-random/seeder#license)

A universal seeder based on [SipHash].

This crate is designed for use with the [rand] crates, allowing any RNG
supporting [`rand_core::SeedableRng`] to be seeded from any hashable value.
It provides the following:

-   `SipHasher` is a portable implementation of SipHash-2-4. According to the
    authors, [SipHash] is a secure, fast and simple keyed hash function.
-   `SipRng` is a PRNG based on the `SipHash` state and mixing operations.
    It is statistically high-quality, passing practrand tests to at least 4 TiB.
-   `SipHasher::into_rng()` transitions a `SipHasher` into a `SipRng`, maintaining
    the full 256 bits of state. (This might break the hasher's security.)
-   `Seeder` is a convenience wrapper around the above (see example).

Seeding is designed to be fast, robust, flexible and portable. This library is
intended for use in simulations and games, allowing e.g. any keyword to
reproduce a simulation or procedurally generated world.

This library is not intended for cryptographic applications, and *definitely*
not for password hashing.

Example:

```rust
use rand_core::RngCore;         // for next_u32
use rand_pcg::Pcg64;            // or whatever you like
use rand_seeder::Seeder;

let mut rng: Pcg64 = Seeder::from("stripy zebra").make_rng();
println!("First value: {}", rng.next_u32());
```

[Changelog](CHANGELOG.md)

[SipHash]: https://131002.net/siphash/
[rand]: https://github.com/rust-random/rand
[`rand_core::SeedableRng`]: https://rust-random.github.io/rand/rand_core/trait.SeedableRng.html

## Rust version requirements

Requires rustc 1.32 or greater for the `.to_le_bytes()` method and for
`rand_core` 0.5 compatibility.

# License

`rand_seeder` is distributed under the terms of both the MIT license and the
Apache License (Version 2.0).

See [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT) for details.
