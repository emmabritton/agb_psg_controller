use agb_fixnum::Num;
use alloc::borrow::Cow;

/// Note field value meaning "no note in this cell".
pub const NOTE_NONE: u8 = 0;
/// Note field value meaning "note off".
pub const NOTE_OFF: u8 = 97;
/// Highest valid semitone index (B-9); C-2 is 1.
pub const NOTE_MAX: u8 = 96;
/// The wave channel plays an octave lower than square at the same period, so the
/// player looks up wave notes with this offset added. Wave notes must therefore
/// not exceed `NOTE_MAX - WAVE_NOTE_OFFSET`.
pub const WAVE_NOTE_OFFSET: u8 = 12;
/// Noise notes index a 60-entry pitch table rather than the period table.
pub const NOISE_NOTE_MAX: u8 = 60;

/// Number of PSG channels in a song: square+sweep, square, wave, noise.
pub const NUM_CHANNELS: usize = 4;

/// A complete song, stored in ROM by `include_pmus!`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track {
    pub instruments: Cow<'static, [Instrument]>,
    pub wave_tables: Cow<'static, [[u8; 16]]>,
    /// Flat cell array, row-major: `row * NUM_CHANNELS + channel`.
    pub pattern_data: Cow<'static, [PatternSlot]>,
    /// Row windows into `pattern_data`.
    pub patterns: Cow<'static, [Pattern]>,
    pub pattern_order: Cow<'static, [u8]>,
    pub frames_per_tick: Num<u32, 8>,
    pub ticks_per_row: u32,
    /// Order index to loop back to after the last entry; `None` = play once and stop.
    pub loop_order_index: Option<u32>,
}

/// A one-channel sound effect, stored in ROM by `include_psfx!`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sfx {
    pub channel: SfxChannel,
    pub instruments: Cow<'static, [Instrument]>,
    pub wave_tables: Cow<'static, [[u8; 16]]>,
    /// One cell per row.
    pub rows: Cow<'static, [PatternSlot]>,
    pub frames_per_tick: Num<u32, 8>,
    pub ticks_per_row: u32,
}

/// The hardware channel a sound effect plays on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SfxChannel {
    SquareSweep,
    Square,
    Wave,
    Noise,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pattern {
    /// First row of this pattern within `Track::pattern_data` (in rows, not slots).
    pub start_row: u32,
    pub num_rows: u32,
}

/// One cell: what happens on one channel in one row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PatternSlot {
    /// `NOTE_NONE`, `1..=NOTE_MAX`, or `NOTE_OFF`.
    pub note: u8,
    /// 1-based index into `instruments`; 0 = keep the current instrument.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PsgEffect {
    None,
    /// Rotate between base note, +x and +y semitones each tick.
    Arpeggio(u8, u8),
    /// Period units per tick.
    PortamentoUp(u8),
    /// Period units per tick.
    PortamentoDown(u8),
    /// Slide toward the cell's note at this many period units per tick.
    TonePortamento(u8),
    Vibrato {
        speed: u8,
        depth: u8,
    },
    /// Added to the channel volume at each row boundary.
    VolumeSlide(i8),
    /// Silence the channel at this tick.
    NoteCut(u8),
    /// Trigger the cell's note at this tick instead of tick 0.
    NoteDelay(u8),
    /// Continue at this order index after the current row.
    PositionJump(u8),
    /// Continue at this row of the next order entry after the current row.
    PatternBreak(u8),
    SetTicksPerRow(u8),
    /// 4.4 fixed point frames per tick.
    SetFramesPerTick(Num<u16, 4>),
    /// Square duty 0-3.
    SetDuty(u8),
    SetPan {
        left: bool,
        right: bool,
    },
    /// 0-15 on square/noise; 0-4 on wave (0 mute, 4 = 100%).
    SetVolume(u8),
}

/// An instrument definition. `duty` is 0-3 (12.5/25/50/75%); wave `volume` is the
/// author-facing 0-4 scale (0 mute, 1 = 25%, 2 = 50%, 3 = 75%, 4 = 100%).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Instrument {
    Square {
        duty: u8,
        envelope: EnvelopeSpec,
        sweep: Option<SweepSpec>,
        /// Length counter 1-64; `None` = play until note off.
        length: Option<u8>,
    },
    Wave {
        /// Index into `wave_tables`.
        wave_table: u8,
        volume: u8,
    },
    Noise {
        envelope: EnvelopeSpec,
        /// 7-bit LFSR ("metallic") instead of 15-bit.
        short_lfsr: bool,
    },
}

/// Hardware volume envelope: `initial_volume` 0-15, `step_time` 0-7 in 1/64 s units
/// (0 = envelope off, volume is static).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnvelopeSpec {
    pub initial_volume: u8,
    pub increasing: bool,
    pub step_time: u8,
}

/// Channel 1 hardware frequency sweep: `time` 0-7 in 1/128 s units, `shift` 0-7.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SweepSpec {
    pub time: u8,
    pub decreasing: bool,
    pub shift: u8,
}
