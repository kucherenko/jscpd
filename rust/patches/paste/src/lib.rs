//! `paste` is unmaintained (RUSTSEC-2024-0436). candle's `gemm`, `pulp` and
//! `tokenizers` still depend on it and call only `paste::paste!`, so the
//! workspace patches `paste` with this crate, which hands that macro to
//! pastey, its maintained successor. Remove the patch once no dependency
//! asks for `paste` any more.

#![no_std]

pub use pastey::paste;
