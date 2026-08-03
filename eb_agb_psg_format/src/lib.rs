use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SongFile {
    pub version: u32,
    pub frames_per_tick: f32,
    pub ticks_per_row: u32,
    pub order: Vec<u32>,
    #[serde(default)]
    pub loop_to: Option<u32>,
    pub instruments: BTreeMap<String, PsgInstrument>,
    #[serde(default)]
    pub waves: BTreeMap<String, String>,
    pub patterns: Vec<Vec<String>>,
}

impl SongFile {
    pub fn from_ron(text: &str) -> Result<Self, ron::error::SpannedError> {
        ron::from_str(text)
    }

    pub fn to_ron(&self) -> Result<String, ron::Error> {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SfxFile {
    pub version: u32,
    pub frames_per_tick: f32,
    pub ticks_per_row: u32,
    pub channel: PsgChannel,
    pub instruments: BTreeMap<String, PsgInstrument>,
    #[serde(default)]
    pub waves: BTreeMap<String, String>,
    pub rows: Vec<String>,
}

impl SfxFile {
    pub fn from_ron(text: &str) -> Result<Self, ron::error::SpannedError> {
        ron::from_str(text)
    }

    pub fn to_ron(&self) -> Result<String, ron::Error> {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PsgChannel {
    SquareSweep,
    Square,
    Wave,
    Noise,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PsgInstrument {
    Square {
        #[serde(default)]
        duty: PsgDuty,
        envelope: (u8, PsgDirection, u8),
        #[serde(default)]
        sweep: Option<(u8, PsgDirection, u8)>,
        #[serde(default)]
        length: Option<u8>,
    },
    Wave {
        table: String,
        volume: PsgWaveVolume,
    },
    Noise {
        envelope: (u8, PsgDirection, u8),
        #[serde(default)]
        lfsr: PsgLfsr,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PsgDuty {
    D12_5,
    D25,
    #[default]
    D50,
    D75,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PsgDirection {
    Up,
    Down,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PsgLfsr {
    Short,
    #[default]
    Long,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PsgWaveVolume {
    V0,
    V25,
    V50,
    V75,
    V100,
}
