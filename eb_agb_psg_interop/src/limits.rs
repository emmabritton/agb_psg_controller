use crate::{NOISE_NOTE_MAX, NOTE_MAX, SfxChannel, WAVE_NOTE_OFFSET};

pub const MAX_ROWS: usize = 256;
pub const MAX_INSTRUMENTS: usize = 255;
pub const MAX_WAVE_TABLES: usize = 255;

pub const TICKS_PER_ROW_MIN: u32 = 1;
pub const TICKS_PER_ROW_MAX: u32 = 31;
pub const NOTE_TICK_MAX: u8 = 31;
pub const DUTY_MAX: u8 = 3;
pub const NIBBLE_PARAM_MAX: u8 = 15;
pub const VOLUME_SLIDE_MAX: i8 = 15;
pub const ENV_VOLUME_MAX: u8 = 15;
pub const ENV_STEP_TIME_MAX: u8 = 7;
pub const SWEEP_TIME_MAX: u8 = 7;
pub const SWEEP_SHIFT_MAX: u8 = 7;
pub const SQUARE_LENGTH_MIN: u8 = 1;
pub const SQUARE_LENGTH_MAX: u8 = 64;

pub const FRAMES_PER_TICK_MIN: u32 = 1;
pub const FRAMES_PER_TICK_MAX: u32 = 255;

pub const fn note_max(channel: SfxChannel) -> u8 {
    match channel {
        SfxChannel::Wave => NOTE_MAX - WAVE_NOTE_OFFSET,
        SfxChannel::Noise => NOISE_NOTE_MAX,
        SfxChannel::SquareSweep | SfxChannel::Square => NOTE_MAX,
    }
}

pub const fn volume_max(channel: SfxChannel) -> u8 {
    match channel {
        SfxChannel::Wave => 4,
        _ => 15,
    }
}

pub const fn duty_allowed(channel: SfxChannel) -> bool {
    matches!(channel, SfxChannel::SquareSweep | SfxChannel::Square)
}

pub const fn period_effects_allowed(channel: SfxChannel) -> bool {
    !matches!(channel, SfxChannel::Noise)
}

pub const fn pitch_slides_allowed(channel: SfxChannel, sweep_active: bool) -> bool {
    period_effects_allowed(channel) && !sweep_active
}
