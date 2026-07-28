//! Music and sound-effect player for the Game Boy Advance's four PSG channels
//! (square + sweep, square, wave, noise), designed to pair with [agb] the same
//! way `agb-tracker` pairs with agb's software mixer.
//!
//! Songs (`.pmus`) and sound effects (`.psfx`) are written in a RON-based text
//! format (see `docs/format-spec.md`) and parsed at compile time into ROM data:
//!
//! ```ignore
//! use eb_agb_psg_controller::{include_pmus, include_psfx, Player, Sfx, Track};
//!
//! static SONG: Track = include_pmus!("assets/song.pmus");
//! static JUMP: Sfx = include_psfx!("assets/jump.psfx");
//!
//! #[agb::entry]
//! fn main(mut gba: agb::Gba) -> ! {
//!     let mut player = Player::new(&SONG);
//!     loop {
//!         player.play_sfx(&JUMP); // steals a channel; music resumes after
//!         player.frame();         // call once per frame
//!         // ...display frame commit...
//!     }
//! }
//! ```
//!
//! # Hardware notes
//!
//! - **No timers**: the player is driven entirely by the per-frame
//!   [`Player::frame`] call, so all four hardware timers stay free (agb's
//!   mixer takes 0 and 1; flash storage commonly needs another).
//! - **agb mixer coexistence**: creating agb's mixer rewrites `SOUNDCNT_H`,
//!   which zeroes the PSG mix-ratio bits. [`Player::frame`] re-asserts them
//!   every frame, so creation order does not matter. Use
//!   [`Player::with_psg_ratio`] to balance PSG against sampled audio.
//! - **Volume quirk**: on the square and noise channels the hardware only
//!   applies volume writes when the channel restarts, so mid-note volume
//!   changes (`Mxx`/`Sxx`) retrigger the note (a small click). Prefer hardware
//!   envelopes for fades — they cost no CPU and click-free.

#![no_std]
#![no_main]
#![cfg_attr(test, feature(custom_test_frameworks))]
#![cfg_attr(test, reexport_test_harness_main = "test_main")]
#![cfg_attr(test, test_runner(agb::test_runner::test_runner))]

extern crate alloc;

// The macros emit paths through `::eb_agb_psg_controller`, which within this
// crate's own tests only resolves via this alias.
// #[cfg(test)]
// extern crate self as eb_agb_psg_controller;

pub use eb_agb_psg_interop::{
    EnvelopeSpec, Instrument, NOISE_NOTE_MAX, NOTE_MAX, NOTE_NONE, NOTE_OFF, NUM_CHANNELS, Pattern,
    PatternSlot, PsgEffect, Sfx, SfxChannel, SweepSpec, Track, WAVE_NOTE_OFFSET,
};
pub use eb_agb_psg_macros::{include_pmus, include_psfx};

mod channels;
mod effects;
mod player;
mod registers;

pub use player::{Player, PsgRatio};

mod dirty {
    pub const RETRIGGER: u8 = 1 << 0;
    pub const SILENCE: u8 = 1 << 1;
    pub const PAN: u8 = 1 << 2;
    pub const PERIOD: u8 = 1 << 3;
    /// Wave channel only: volume changed without a retrigger.
    pub const VOLUME: u8 = 1 << 4;
}

struct ChannelState {
    /// 1-based index into the owning instrument list; 0 = none yet.
    pub instrument: u8,
    /// The note currently sounding, or 0 when the channel is silent (before its
    /// first note, or after a note off / note cut). Effects that would restart
    /// the channel check this first — see [`silence_state`].
    pub note: u8,
    /// Effective noise pitch for this tick (differs from `note` under arpeggio).
    pub noise_note: u8,
    pub period: u16,
    /// Tone portamento destination.
    pub target_period: u16,
    /// Logical volume: 0-15 on square/noise, 0-4 on wave.
    pub volume: u8,
    pub duty: u8,
    pub pan_left: bool,
    pub pan_right: bool,
    pub vibrato_phase: u8,
    pub vib_offset: i16,
    /// The current row's effect, active until the next row.
    pub effect: PsgEffect,
    /// A cell deferred by a note delay (`Qxx`).
    pub delayed: Option<PatternSlot>,
    /// Wave channel only: which wave table is currently in wave RAM.
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
            effect: PsgEffect::None,
            delayed: None,
            loaded_wave: None,
            dirty: 0,
        }
    }
}

/// Ends whatever is sounding on a channel. Clearing the note is what stops a
/// later `Mxx`/`Sxx`/`Wxx` from retriggering it: those effects only reach
/// hardware while a note is live (a silenced channel just remembers the new
/// value for its next note-on).
fn silence_state(state: &mut ChannelState) {
    state.note = 0;
    state.noise_note = 0;
    // A pending pan change describes the channel, not the note, so keep it.
    state.dirty = (state.dirty & dirty::PAN) | dirty::SILENCE;
}

#[doc(hidden)]
pub mod __private {
    pub use agb::fixnum::Num;
    pub use alloc::borrow::Cow;
    pub use eb_agb_psg_interop;
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_SONG: Track = include_pmus!("assets/test_song.pmus");
    static TEST_SFX: Sfx = include_psfx!("assets/sfx/test_blip.psfx");
    static EFFECTS_SONG: Track = include_pmus!("assets/test_effects.pmus");
    static TEMPO_SONG: Track = include_pmus!("assets/test_tempo.pmus");
    static SILENCE_SONG: Track = include_pmus!("assets/test_silence.pmus");

    /// Throwaway cycle-count benchmark for `Player::frame()`, not a
    /// correctness test. Uses timer2 (cycle-precise) cascaded into timer3 for
    /// a wraparound-safe 32-bit counter, per agb's `Timers` API.
    #[test_case]
    fn frame_cost_benchmark(gba: &mut agb::Gba) {
        use agb::timer::Divider;

        const ITERATIONS: u32 = 500;

        let mut timers = gba.timers.timers();
        timers.timer2.set_divider(Divider::Divider1);
        timers.timer3.set_cascade(true);
        timers.timer2.set_enabled(true);
        timers.timer3.set_enabled(true);

        // Each measurement is a difference: `set_overflow_amount` only writes
        // the reload value, so it cannot zero a timer that is already running.
        // Reading the two halves is not atomic either -- if timer2 wraps
        // between them the pair is inconsistent, so re-read until the high
        // half agrees with itself.
        let counter = || loop {
            let high = timers.timer3.value();
            let low = timers.timer2.value();
            if high == timers.timer3.value() {
                return ((high as u32) << 16) | low as u32;
            }
        };

        // Idle-ish: TEST_SONG runs at 2.0 frames/tick, so half the calls do
        // no row/tick work at all -- mostly register-reassertion + the
        // fast-path skips in step_sfx/realise.
        let mut idle_player = Player::new(&TEST_SONG);
        let start = counter();
        for _ in 0..ITERATIONS {
            idle_player.frame();
        }
        let idle_cycles = counter().wrapping_sub(start);

        // Busy: EFFECTS_SONG runs at 1 frame/tick, so every call does real
        // row/tick/effect processing across all four channels.
        let mut busy_player = Player::new(&EFFECTS_SONG);
        let start = counter();
        for _ in 0..ITERATIONS {
            busy_player.frame();
        }
        let busy_cycles = counter().wrapping_sub(start);

        agb::println!(
            "BENCH frame() avg cycles over {} calls -- idle: {}, busy: {}",
            ITERATIONS,
            idle_cycles / ITERATIONS,
            busy_cycles / ITERATIONS
        );
    }

    #[test_case]
    fn song_is_embedded_in_rom(_gba: &mut agb::Gba) {
        assert_eq!(TEST_SONG.instruments.len(), 3);
        assert_eq!(TEST_SONG.wave_tables.len(), 1);
        assert_eq!(TEST_SONG.patterns.len(), 1);
        assert_eq!(TEST_SONG.patterns[0].num_rows, 4);
        assert_eq!(TEST_SONG.pattern_data.len(), 4 * NUM_CHANNELS);
        assert_eq!(TEST_SONG.pattern_order.as_ref(), &[0, 0]);
        assert_eq!(TEST_SONG.loop_order_index, Some(0));
        assert_eq!(TEST_SONG.ticks_per_row, 4);
        // 2.0 frames per tick in 8.8 fixed point
        assert_eq!(TEST_SONG.frames_per_tick.to_raw(), 512);

        // row 0: C-4 on the square channel, C-3 on wave, C-5 on noise
        assert_eq!(TEST_SONG.pattern_data[1].note, 25);
        assert_eq!(TEST_SONG.pattern_data[2].note, 13);
        assert_eq!(TEST_SONG.pattern_data[3].note, 37);
        // row 3: note off on the square channel
        assert_eq!(TEST_SONG.pattern_data[3 * NUM_CHANNELS + 1].note, NOTE_OFF);
    }

    #[test_case]
    fn player_advances_rows_at_the_right_tempo(_gba: &mut agb::Gba) {
        // TEST_SONG: 2.0 frames per tick, 4 ticks per row -> a row every 8 frames.
        let mut player = Player::new(&TEST_SONG);
        assert_eq!(player.debug_position(), (0, 0, 0));

        for _ in 0..8 {
            player.frame();
        }
        assert_eq!(player.debug_position(), (0, 1, 0));

        // 4 rows per pattern: 32 frames ends pattern 0, moving to order position 1.
        for _ in 0..24 {
            player.frame();
        }
        assert_eq!(player.debug_position(), (1, 0, 0));

        // After both order entries the song loops back to order position 0.
        for _ in 0..32 {
            player.frame();
        }
        assert_eq!(player.debug_position(), (0, 0, 0));
        assert!(!player.is_finished());
    }

    #[test_case]
    fn effects_drive_channel_state(_gba: &mut agb::Gba) {
        // EFFECTS_SONG: 1 frame per tick, 4 ticks per row. C-4 square is
        // period 1547; C-3 wave looks up the same table entry.
        let mut player = Player::new(&EFFECTS_SONG);

        player.frame(); // row 0 tick 0
        assert_eq!(player.debug_channel(0).0, 1547);
        assert_eq!(player.debug_channel(1).0, 1547);
        assert_eq!(player.debug_channel(2).0, 0); // Q02 delays the wave note

        player.frame(); // tick 1
        player.frame(); // tick 2: the delayed wave note fires
        assert_eq!(player.debug_channel(2).0, 1547);

        player.frame(); // tick 3, wraps to row 1
        player.frame(); // row 1 tick 0
        player.frame(); // tick 1: A30 -> +3 semitones (D#4), U02 -> +2 period
        assert_eq!(player.debug_channel(0).0, 1627);
        assert_eq!(player.debug_channel(1).0, 1549);

        player.frame(); // tick 2: A30 -> +0 (back to base)
        assert_eq!(player.debug_channel(0).0, 1547);

        player.frame(); // tick 3: U02 total +6, wraps to row 2
        assert_eq!(player.debug_channel(1).0, 1553);

        player.frame(); // row 2 tick 0: S02 -> volume 12 + 2
        assert_eq!(player.debug_channel(1).1, 14);

        // Rest of row 2, then row 3 whose B00 jumps back to the start.
        for _ in 0..7 {
            player.frame();
        }
        assert_eq!(player.debug_position(), (0, 0, 0));
    }

    #[test_case]
    fn tempo_effect_changes_row_length(_gba: &mut agb::Gba) {
        // TEMPO_SONG: R02 on the first row drops ticks-per-row from 4 to 2.
        let mut player = Player::new(&TEMPO_SONG);
        player.frame();
        player.frame();
        assert_eq!(player.debug_position(), (0, 1, 0));
        player.frame();
        player.frame();
        assert_eq!(player.debug_position(), (0, 0, 0));
    }

    #[test_case]
    fn sfx_steals_and_releases_channel(_gba: &mut agb::Gba) {
        // TEST_SFX plays on the square channel (index 1): 2 rows of 2 ticks at
        // 1 frame per tick = 4 frames long.
        let mut player = Player::new(&TEST_SONG);
        player.play_sfx(&TEST_SFX);
        assert!(player.debug_sfx_active(1));

        for _ in 0..3 {
            player.frame();
        }
        assert!(player.debug_sfx_active(1));
        player.frame();
        assert!(!player.debug_sfx_active(1));

        // Music kept advancing while the channel was stolen: TEST_SONG rows
        // are 8 frames, so 8 total frames = row 1.
        for _ in 0..4 {
            player.frame();
        }
        assert_eq!(player.debug_position(), (0, 1, 0));
    }

    #[test_case]
    fn sfx_only_player_needs_no_song(_gba: &mut agb::Gba) {
        let mut player = Player::sfx_only();
        assert!(player.is_finished());
        assert_eq!(player.debug_position(), (0, 0, 0));

        player.play_sfx(&TEST_SFX);
        assert!(player.debug_sfx_active(1));
        for _ in 0..4 {
            player.frame();
        }
        assert!(!player.debug_sfx_active(1));

        // A song can still be started later.
        player.play_song(&TEST_SONG);
        assert!(!player.is_finished());
        for _ in 0..8 {
            player.frame();
        }
        assert_eq!(player.debug_position(), (0, 1, 0));
        // The first row's note survived the song-switch silence.
        assert_eq!(player.debug_channel(1).0, 1547); // C-4
    }

    /// Volume effects on a channel with nothing sounding must not touch
    /// hardware: on noise a retrigger there used to index the pitch table at
    /// note 0 and panic, and on square it restarted a note the song had ended.
    #[test_case]
    fn volume_effects_on_silent_channels_do_not_trigger(_gba: &mut agb::Gba) {
        use crate::registers::{SOUND2CNT_L, SOUND4CNT_L};

        // SILENCE_SONG is 1 frame per row.
        let mut player = Player::new(&SILENCE_SONG);

        // Row 0 sets volumes with no note in sight.
        player.frame();
        assert_eq!(player.debug_channel(1).1, 4);
        assert_eq!(player.debug_channel(3).1, 8);
        assert_eq!(SOUND2CNT_L.get(), 0);
        assert_eq!(SOUND4CNT_L.get(), 0);

        // Row 1 plays both, using the volumes stored above (envelope byte in
        // the high half: volume << 4, then duty 2 for the square channel).
        player.frame();
        assert_eq!(SOUND2CNT_L.get(), 0x4080);
        assert_eq!(SOUND4CNT_L.get(), 0x8000);

        // Row 2 is a note off on both.
        player.frame();
        assert_eq!(SOUND2CNT_L.get(), 0);
        assert_eq!(SOUND4CNT_L.get(), 0);

        // Row 3 changes volume on both silenced channels: remembered for the
        // next note-on, but no retrigger.
        player.frame();
        assert_eq!(player.debug_channel(1).1, 15);
        assert_eq!(player.debug_channel(3).1, 2);
        assert_eq!(SOUND2CNT_L.get(), 0);
        assert_eq!(SOUND4CNT_L.get(), 0);
    }

    #[test_case]
    fn coexists_with_agb_mixer(gba: &mut agb::Gba) {
        use crate::registers::{MASTER_ENABLE, PSG_RATIO_MASK, SOUNDCNT_H, SOUNDCNT_X};

        let mut player = Player::new(&TEST_SONG);
        assert_ne!(SOUNDCNT_X.get() & MASTER_ENABLE, 0);
        assert_eq!(SOUNDCNT_H.get() & PSG_RATIO_MASK, PsgRatio::Full as u16);

        // Creating agb's mixer rewrites SOUNDCNT_H (PSG ratio bits become 0);
        // the next player.frame() must restore them.
        let mut mixer = gba.mixer.mixer(agb::sound::mixer::Frequency::Hz10512);
        mixer.frame();
        player.frame();
        assert_eq!(SOUNDCNT_H.get() & PSG_RATIO_MASK, PsgRatio::Full as u16);
        assert_ne!(SOUNDCNT_X.get() & MASTER_ENABLE, 0);
    }

    #[test_case]
    fn sfx_is_embedded_in_rom(_gba: &mut agb::Gba) {
        assert_eq!(TEST_SFX.channel, SfxChannel::Square);
        assert_eq!(TEST_SFX.rows.len(), 2);
        assert_eq!(TEST_SFX.rows[0].note, 49); // C-6
        assert_eq!(TEST_SFX.rows[1].note, NOTE_OFF);
    }
}

#[allow(clippy::empty_loop)]
#[cfg(test)]
#[agb::entry]
fn main(mut gba: agb::Gba) -> ! {
    loop {}
}
