//! The `.pmus` song / `.psfx` sound-effect file format used by
//! [`agb_psg_controller`](https://github.com/emmabritton/agb_psg_controller):
//! the serde data model plus RON (de)serialization.
//!
//! This crate is for host-side tooling (editors, converters, generators) — it
//! has no GBA dependencies. Reading a file here only checks that it is
//! structurally valid RON; semantic validation (note ranges, channel/instrument
//! matching, jump targets) happens when the file is lowered for playback by
//! `agb_psg_interop` / the `include_pmus!` macro.
//!
//! The format itself is documented in `docs/format-spec.md` in the repository.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The current file format version. Files with a different `version` field are
/// rejected by the playback parser.
pub const FORMAT_VERSION: u32 = 1;

/// A song document: four channels (square+sweep, square, wave, noise).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SongFile {
    /// Must be [`FORMAT_VERSION`].
    pub version: u32,
    /// Frames (1/59.73 s) per tick; fractional values allowed (stored as 8.8
    /// fixed point on the GBA). Must be > 0 and ≤ 255.
    pub frames_per_tick: f32,
    /// Initial ticks per pattern row, 1-31.
    pub ticks_per_row: u32,
    /// Pattern play order (indices into `patterns`).
    pub order: Vec<u32>,
    /// Order index to loop back to after the last entry; `None` = play once.
    #[serde(default)]
    pub loop_to: Option<u32>,
    pub instruments: BTreeMap<String, PsgInstrument>,
    /// Wave tables by name: exactly 32 hex digits each (32 4-bit samples,
    /// high nibble first).
    #[serde(default)]
    pub waves: BTreeMap<String, String>,
    /// Patterns of tracker-style row strings: 4 cells separated by `|`
    /// (`note instrument effect`), or `skip N` for N empty rows.
    pub patterns: Vec<Vec<String>>,
}

impl SongFile {
    /// Reads a song document from RON text (structural check only).
    pub fn from_ron(text: &str) -> Result<Self, ron::error::SpannedError> {
        ron::from_str(text)
    }

    /// Serializes this song document to RON text.
    pub fn to_ron(&self) -> Result<String, ron::Error> {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
    }
}

/// A one-channel sound-effect document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SfxFile {
    /// Must be [`FORMAT_VERSION`].
    pub version: u32,
    /// See [`SongFile::frames_per_tick`].
    pub frames_per_tick: f32,
    /// Ticks per row, 1-31.
    pub ticks_per_row: u32,
    /// The hardware channel this effect plays on.
    pub channel: PsgChannel,
    pub instruments: BTreeMap<String, PsgInstrument>,
    #[serde(default)]
    pub waves: BTreeMap<String, String>,
    /// Tracker-style row strings, one cell each (or `skip N`).
    pub rows: Vec<String>,
}

impl SfxFile {
    /// Reads a sound-effect document from RON text (structural check only).
    pub fn from_ron(text: &str) -> Result<Self, ron::error::SpannedError> {
        ron::from_str(text)
    }

    /// Serializes this sound-effect document to RON text.
    pub fn to_ron(&self) -> Result<String, ron::Error> {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
    }
}

/// A PSG hardware channel, as referenced by an SFX document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PsgChannel {
    SquareSweep,
    Square,
    Wave,
    Noise,
}

/// An instrument definition as authored in the file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PsgInstrument {
    Square {
        #[serde(default)]
        duty: PsgDuty,
        /// (initial volume 0-15, direction, step time 0-7; 0 = static)
        envelope: (u8, PsgDirection, u8),
        /// (time 0-7, direction, shift 0-7); usable on channel 1 only.
        #[serde(default)]
        sweep: Option<(u8, PsgDirection, u8)>,
        /// Length counter 1-64; `None` = play until note off.
        #[serde(default)]
        length: Option<u8>,
    },
    Wave {
        /// Name of an entry in the document's `waves` map.
        table: String,
        volume: PsgWaveVolume,
    },
    Noise {
        /// (initial volume 0-15, direction, step time 0-7; 0 = static)
        envelope: (u8, PsgDirection, u8),
        #[serde(default)]
        lfsr: PsgLfsr,
    },
}

/// Square-channel duty cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PsgDuty {
    D12_5,
    D25,
    #[default]
    D50,
    D75,
}

/// Envelope/sweep direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PsgDirection {
    Up,
    Down,
}

/// Noise LFSR width: `Short` (7-bit, metallic) or `Long` (15-bit).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PsgLfsr {
    Short,
    #[default]
    Long,
}

/// Wave-channel output level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PsgWaveVolume {
    V0,
    V25,
    V50,
    V75,
    V100,
}
