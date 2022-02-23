//! Test tool
//!
//! Can be used with practrand like so, where `HASHABLES` is optional input:
//! ```
//! cargo run --release --example seedrng -- -s HASHABLES | practrand stdin64
//! ```

extern crate rand_seeder;

use rand_seeder::rand_core::RngCore;
use rand_seeder::SipHasher;

use std::env;
use std::hash::Hasher;
use std::io::{self, Write};

fn print_usage() {
    println!("Usage: seedrng [OPTIONS] SEED...");
    println!("Constructs a pseudo-random number generator from the given");
    println!("input and yields random output.");
    println!("");
    println!("Options:");
    println!("\t-h\tPrint this text");
    println!(
        "\t-s\tOutput a continuous stream, suitable for use with Practrand (use stdin64 option)"
    );
}

enum Output {
    Single,
    Stream,
}

fn main() {
    let mut output = Output::Single;
    let mut hasher = SipHasher::new();

    for arg in env::args().skip(1) {
        if arg.starts_with("-") {
            match arg.as_str() {
                "-h" => return print_usage(),
                "-s" => output = Output::Stream,
                o @ _ => {
                    eprintln!("Error: unrecognised option {}", o);
                    return;
                }
            }
        } else {
            hasher.write(arg.as_bytes());
        }
    }

    let mut rng = hasher.into_rng();
    match output {
        Output::Single => {
            println!("{}", rng.next_u64());
        }
        Output::Stream => {
            let stdout = io::stdout();
            let mut handle = stdout.lock();
            let mut buf = [0u8; 32];

            loop {
                rng.fill_bytes(&mut buf);
                handle.write_all(&buf).unwrap();
            }
        }
    }
}
