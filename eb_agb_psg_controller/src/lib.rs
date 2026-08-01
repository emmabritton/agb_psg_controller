#![no_std]
#![cfg_attr(target_arch = "arm", no_main)]
#![cfg_attr(test, feature(custom_test_frameworks))]
#![cfg_attr(test, reexport_test_harness_main = "test_main")]
#![cfg_attr(test, test_runner(agb::test_runner::test_runner))]

extern crate alloc;

#[cfg(test)]
extern crate self as eb_agb_psg_controller;

pub use eb_agb_psg_interop::{
    EnvelopeSpec, FrameCount, Instrument, NOISE_NOTE_MAX, NOTE_MAX, NOTE_NONE, NOTE_OFF,
    NUM_CHANNELS, Pattern, PatternSlot, PsgEffect, Sfx, SfxChannel, SweepSpec, Track,
    WAVE_NOTE_OFFSET, limits,
};
pub use eb_agb_psg_macros::{include_pmus, include_psfx};

mod channels;
mod effects;
mod player;
mod registers;

pub use player::{Player, PsgRatio};

#[cfg(feature = "host")]
pub use registers::host;

mod dirty {
    pub const RETRIGGER: u8 = 1 << 0;
    pub const SILENCE: u8 = 1 << 1;
    pub const PAN: u8 = 1 << 2;
    pub const PERIOD: u8 = 1 << 3;
    pub const VOLUME: u8 = 1 << 4;
}

struct ChannelState {
    pub instrument: u8,
    pub note: u8,
    pub noise_note: u8,
    pub period: u16,
    pub target_period: u16,
    pub volume: u8,
    pub duty: u8,
    pub pan_left: bool,
    pub pan_right: bool,
    pub vibrato_phase: u8,
    pub vib_offset: i16,
    pub arp_step: u8,
    pub effect: PsgEffect,
    pub delayed: Option<PatternSlot>,
    pub loaded_wave: Option<u8>,
    pub dirty: u8,
}

impl Default for ChannelState {
    fn default() -> Self {
        Self {
            instrument: 0,
            note: 0,
            noise_note: 0,
            period: 0,
            target_period: 0,
            volume: 0,
            duty: 0,
            pan_left: true,
            pan_right: true,
            vibrato_phase: 0,
            vib_offset: 0,
            arp_step: 0,
            effect: PsgEffect::None,
            delayed: None,
            loaded_wave: None,
            dirty: 0,
        }
    }
}

fn silence_state(state: &mut ChannelState) {
    state.note = 0;
    state.noise_note = 0;
    // A pending pan change describes the channel, not the note, so keep it.
    state.dirty = (state.dirty & dirty::PAN) | dirty::SILENCE;
}

#[doc(hidden)]
pub mod __private {
    pub use agb_fixnum::Num;
    pub use alloc::borrow::Cow;
    pub use eb_agb_psg_interop;
}

#[allow(clippy::empty_loop)]
#[cfg(test)]
#[agb::entry]
fn main(mut gba: agb::Gba) -> ! {
    loop {}
}
