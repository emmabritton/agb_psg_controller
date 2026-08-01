use agb_fixnum::Num;
use alloc::borrow::Cow;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

pub const NOTE_NONE: u8 = 0;
pub const NOTE_OFF: u8 = 97;
pub const NOTE_MAX: u8 = 96;
pub const WAVE_NOTE_OFFSET: u8 = 12;
pub const NOISE_NOTE_MAX: u8 = 60;

pub const NUM_CHANNELS: usize = 4;

pub type FrameCount = Num<u32, 8>;

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track {
    pub instruments: Cow<'static, [Instrument]>,
    pub wave_tables: Cow<'static, [[u8; 16]]>,
    pub pattern_data: Cow<'static, [PatternSlot]>,
    pub patterns: Cow<'static, [Pattern]>,
    pub pattern_order: Cow<'static, [u8]>,
    pub frames_per_tick: FrameCount,
    pub ticks_per_row: u32,
    pub loop_order_index: Option<u32>,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sfx {
    pub channel: SfxChannel,
    pub instruments: Cow<'static, [Instrument]>,
    pub wave_tables: Cow<'static, [[u8; 16]]>,
    pub rows: Cow<'static, [PatternSlot]>,
    pub frames_per_tick: FrameCount,
    pub ticks_per_row: u32,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SfxChannel {
    SquareSweep,
    Square,
    Wave,
    Noise,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pattern {
    pub start_row: u32,
    pub num_rows: u32,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PatternSlot {
    pub note: u8,
    // 1 based index 
    pub instrument: u8,
    pub effect: PsgEffect,
}

impl PatternSlot {
    pub const EMPTY: PatternSlot = PatternSlot {
        note: NOTE_NONE,
        instrument: 0,
        effect: PsgEffect::None,
    };
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PsgEffect {
    None,
    Arpeggio(u8, u8),
    PortamentoUp(u8),
    PortamentoDown(u8),
    TonePortamento(u8),
    Vibrato {
        speed: u8,
        depth: u8,
    },
    VolumeSlide(i8),
    NoteCut(u8),
    NoteDelay(u8),
    PositionJump(u8),
    PatternBreak(u8),
    SetTicksPerRow(u8),
    SetFramesPerTick(Num<u16, 4>),
    SetDuty(u8),
    SetPan {
        left: bool,
        right: bool,
    },
    SetVolume(u8),
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Instrument {
    Square {
        duty: u8,
        envelope: EnvelopeSpec,
        sweep: Option<SweepSpec>,
        length: Option<u8>,
    },
    Wave {
        wave_table: u8,
        volume: u8,
    },
    Noise {
        envelope: EnvelopeSpec,
        short_lfsr: bool,
    },
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnvelopeSpec {
    pub initial_volume: u8,
    pub increasing: bool,
    pub step_time: u8,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SweepSpec {
    pub time: u8,
    pub decreasing: bool,
    pub shift: u8,
}
