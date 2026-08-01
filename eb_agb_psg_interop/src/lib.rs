#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

mod track;
pub use track::*;

pub mod limits;

mod tables;
pub use tables::{NOISE_TABLE, NOTE_PERIODS};

#[cfg(feature = "parse")]
pub mod parse;

#[cfg(feature = "parse")]
pub mod emit;

#[cfg(feature = "quote")]
mod tokens;
