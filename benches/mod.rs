// Copyright 2019 Developers of the Rand project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(test)]

extern crate rand_seeder;
extern crate test;

const RAND_BENCH_N: u64 = 1000;
const BYTES_LEN: usize = 1024;

use std::mem::size_of;
use test::{black_box, Bencher};

use rand_seeder::rand_core::RngCore;
use rand_seeder::{Seeder, SipRng};

#[bench]
fn gen_bytes_siprng(b: &mut Bencher) {
    let mut rng: SipRng = Seeder::from("siprng").make_rng();
    let mut buf = [0u8; BYTES_LEN];
    b.iter(|| {
        for _ in 0..RAND_BENCH_N {
            rng.fill_bytes(&mut buf);
            black_box(buf);
        }
    });
    b.bytes = BYTES_LEN as u64 * RAND_BENCH_N;
}

macro_rules! gen_uint {
    ($fnn:ident, $ty:ty, $m:ident, $gen:expr) => {
        #[bench]
        fn $fnn(b: &mut Bencher) {
            let mut rng: SipRng = $gen;
            b.iter(|| {
                let mut accum: $ty = 0;
                for _ in 0..RAND_BENCH_N {
                    accum = accum.wrapping_add(rng.$m());
                }
                accum
            });
            b.bytes = size_of::<$ty>() as u64 * RAND_BENCH_N;
        }
    };
}

gen_uint!(
    gen_u32_siprng,
    u32,
    next_u32,
    Seeder::from("siprng").make_rng()
);
gen_uint!(
    gen_u64_siprng,
    u64,
    next_u64,
    Seeder::from("siprng").make_rng()
);
