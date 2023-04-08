# ndarray-npz

[![Build][]](https://github.com/qu1x/ndarray-npz/actions/workflows/build.yml)
[![Documentation][]](https://docs.rs/ndarray-npz)
[![Downloads][]](https://crates.io/crates/ndarray-npz)
[![Version][]](https://crates.io/crates/ndarray-npz)
[![Rust][]](https://www.rust-lang.org)
[![License][]](https://opensource.org/licenses)

[Build]: https://github.com/qu1x/ndarray-npz/actions/workflows/build.yml/badge.svg
[Documentation]: https://docs.rs/ndarray-npz/badge.svg
[Downloads]: https://img.shields.io/crates/d/ndarray-npz.svg
[Version]: https://img.shields.io/crates/v/ndarray-npz.svg
[Rust]: https://img.shields.io/badge/rust-v1.60-brightgreen.svg
[License]: https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg

Advanced [`.npz`] file format support for [`ndarray`].

## Accessing [`.npy`] Files

  * See [`ndarray_npy`].

## Accessing [`.npz`] Files

  * Reading: [`NpzReader`]
  * Writing: [`NpzWriter`]
  * Immutable viewing (primarily for use with memory-mapped files):
      * [`NpzView`] providing an [`NpyView`] for each uncompressed [`.npy`] file within
        the archive
  * Mutable viewing (primarily for use with memory-mapped files):
      * [`NpzViewMut`] providing an [`NpyViewMut`] for each uncompressed [`.npy`] file within
        the archive

[`.npy`]: https://numpy.org/doc/stable/reference/generated/numpy.lib.format.html
[`.npz`]: https://numpy.org/doc/stable/reference/generated/numpy.savez.html

[`ndarray`]: https://docs.rs/ndarray
[`ndarray_npy`]: https://docs.rs/ndarray_npy

[`NpzReader`]: https://docs.rs/ndarray-npz/latest/ndarray_npz/struct.NpzReader.html
[`NpzWriter`]: https://docs.rs/ndarray-npz/latest/ndarray_npz/struct.NpzWriter.html
[`NpzView`]: https://docs.rs/ndarray-npz/latest/ndarray_npz/struct.NpzView.html
[`NpyView`]: https://docs.rs/ndarray-npz/latest/ndarray_npz/struct.NpyView.html
[`NpzViewMut`]: https://docs.rs/ndarray-npz/latest/ndarray_npz/struct.NpzViewMut.html
[`NpyViewMut`]: https://docs.rs/ndarray-npz/latest/ndarray_npz/struct.NpyViewMut.html

## Releases

See the [release history](RELEASES.md) to keep track of the development.

## Features

Both features are enabled by default.

  * `compressed`: Enables zip archives with *deflate* compression.
  * `num-complex-0_4`: Enables complex element types of crate `num-complex`.

# License

Copyright © 2021-2023 Rouven Spreckels <rs@qu1x.dev>

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSES/Apache-2.0](LICENSES/Apache-2.0) or
   https://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSES/MIT](LICENSES/MIT) or https://opensource.org/licenses/MIT)

at your option.

# Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this project by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.
