#![doc = include_str!("../README.md")]
#![no_std]
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(rustdoc::private_intra_doc_links)]
#![deny(rustdoc::invalid_rust_codeblocks)]
#![deny(rustdoc::bare_urls)]
#![deny(rustdoc::redundant_explicit_links)]

mod encoder;
mod parser;
mod types;

extern crate alloc;

pub use encoder::EncodeError;
pub use parser::ParserSettings;
pub use types::*;
