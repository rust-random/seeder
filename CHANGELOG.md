# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.3] - 2022-02-24
Fix undefined behaviour caused by narrowing provenance of slice pointer to a
single element then reading multiple elements (#8).

## [0.2.2] - 2020-12-23
Support `rand_core` v0.6
Migrate to GitHub Actions CI

## [0.2.1] - 2020-05-22
Add `Seeder::make_seed`.

## [0.2.0] - 2019-11-04
Documentation reviewed and improved.

### Changes
-   Rename `SipHasher::make_rng` → `SipHasher::into_rng`
-   Rename `SipHasher::new_with_keys` → `SipHasher::from_keys`

## [0.1.0] - 2019-10-15
Initial release
