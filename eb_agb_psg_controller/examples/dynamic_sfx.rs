//! Building songs and sound effects at runtime, as an on-device editor would.
//!
//! Run with `cargo run --features dynamic --example dynamic_sfx`.
//!
//! `Track` and `Sfx` are ordinary structs, so nothing stops you filling them in
//! by hand instead of with `include_pmus!`/`include_psfx!`. The catch is
//! ownership: `play_sfx` borrows for the player's lifetime, so a struct that
//! owns *both* the player and the effect cannot hand one to the other. The
//! `dynamic` feature adds `play_sfx_shared`/`play_song_shared`, which take an
//! `Rc` — that is what makes `Tracker` below compile with no lifetime
//! parameter, no leaking, and no unsafe.
//!
//! Controls: Up/Down transpose, Left/Right change tempo, A plays the effect,
//! Start plays the generated song, Select stops everything.

#![no_std]
#![no_main]

extern crate alloc;

use agb::input::{Button, ButtonController};
use alloc::borrow::Cow;
use alloc::rc::Rc;
use alloc::vec;
use alloc::vec::Vec;
use eb_agb_psg_controller::{
    EnvelopeSpec, FrameCount, Instrument, NOTE_MAX, NUM_CHANNELS, Pattern, PatternSlot, Player,
    PsgEffect, Sfx, SfxChannel, Track,
};

/// The editor's document: plain owned data, no `Sfx`, no lifetimes. This is
/// what a real editor would mutate as the user moves around the grid, and what
/// it would serialise for export.
struct Document {
    note: u8,
    /// Whole frames per tick. The player takes a `FrameCount` (8.8 fixed
    /// point), so a real editor would expose fractional tempos too.
    frames_per_tick: u32,
}

impl Document {
    fn new() -> Self {
        Self {
            note: 49, // C-6
            frames_per_tick: 2,
        }
    }

    /// Transposing and tempo changes clamp here so the document can never hold
    /// a value the player would choke on: hand-built data skips the `.psfx`
    /// parser, which is what would normally reject these.
    fn transpose(&mut self, delta: i8) {
        self.note = (self.note as i16 + delta as i16).clamp(1, NOTE_MAX as i16) as u8;
    }

    fn adjust_tempo(&mut self, delta: i32) {
        // Zero frames per tick would never finish a frame in the player.
        self.frames_per_tick = (self.frames_per_tick as i32 + delta).clamp(1, 16) as u32;
    }

    fn square_instrument(&self) -> Instrument {
        Instrument::Square {
            duty: 2,
            envelope: EnvelopeSpec {
                initial_volume: 12,
                increasing: false,
                step_time: 2,
            },
            sweep: None,
            length: None,
        }
    }

    /// Materialise the document as a playable effect: a three-row blip that
    /// slides up and fades out.
    fn build_sfx(&self) -> Sfx {
        let rows = vec![
            PatternSlot {
                note: self.note,
                instrument: 1,
                effect: PsgEffect::None,
            },
            PatternSlot {
                note: 0,
                instrument: 0,
                effect: PsgEffect::PortamentoUp(0x20),
            },
            PatternSlot {
                note: 0,
                instrument: 0,
                effect: PsgEffect::VolumeSlide(-6),
            },
        ];

        Sfx {
            channel: SfxChannel::Square,
            instruments: Cow::Owned(vec![self.square_instrument()]),
            wave_tables: Cow::Owned(Vec::new()),
            rows: Cow::Owned(rows),
            frames_per_tick: FrameCount::new(self.frames_per_tick),
            ticks_per_row: 4,
        }
    }

    /// Materialise a one-pattern song: an arpeggio walking up from the
    /// document's note on the second square channel.
    fn build_track(&self) -> Track {
        const ROWS: u32 = 4;

        let mut pattern_data = vec![PatternSlot::EMPTY; ROWS as usize * NUM_CHANNELS];
        for row in 0..ROWS {
            // Channel 1 is the plain square channel (0 is square+sweep).
            pattern_data[row as usize * NUM_CHANNELS + 1] = PatternSlot {
                note: (self.note + row as u8 * 4).min(NOTE_MAX),
                instrument: 1,
                effect: PsgEffect::None,
            };
        }

        Track {
            instruments: Cow::Owned(vec![self.square_instrument()]),
            wave_tables: Cow::Owned(Vec::new()),
            pattern_data: Cow::Owned(pattern_data),
            patterns: Cow::Owned(vec![Pattern {
                start_row: 0,
                num_rows: ROWS,
            }]),
            pattern_order: Cow::Owned(vec![0]),
            frames_per_tick: FrameCount::new(self.frames_per_tick),
            ticks_per_row: 4,
            loop_order_index: Some(0),
        }
    }
}

/// Owns the player *and* everything it plays — the shape the `dynamic` feature
/// exists for. Note the absence of a lifetime parameter: with plain `play_sfx`
/// this struct could not be written, because `Player<'a>` would have to borrow
/// from a sibling field.
struct Tracker {
    player: Player<'static>,
    document: Document,
    sfx: Rc<Sfx>,
    track: Rc<Track>,
}

impl Tracker {
    fn new() -> Self {
        let document = Document::new();
        Self {
            player: Player::new(),
            sfx: Rc::new(document.build_sfx()),
            track: Rc::new(document.build_track()),
            document,
        }
    }

    /// Re-materialise after an edit. The `Rc`s already handed to the player
    /// keep the old versions alive until playback releases them, so this is
    /// safe to call while something is sounding — nothing has to stop first.
    /// By the same token, anything already playing keeps the version it was
    /// given: press A or Start again to hear the edit.
    fn rebuild(&mut self) {
        self.sfx = Rc::new(self.document.build_sfx());
        self.track = Rc::new(self.document.build_track());
        agb::println!(
            "note {} | {} frames per tick",
            self.document.note,
            self.document.frames_per_tick
        );
    }

    fn update(&mut self, input: &ButtonController) {
        let mut edited = false;
        if input.is_just_pressed(Button::Up) {
            self.document.transpose(1);
            edited = true;
        }
        if input.is_just_pressed(Button::Down) {
            self.document.transpose(-1);
            edited = true;
        }
        if input.is_just_pressed(Button::Right) {
            self.document.adjust_tempo(1);
            edited = true;
        }
        if input.is_just_pressed(Button::Left) {
            self.document.adjust_tempo(-1);
            edited = true;
        }
        if edited {
            self.rebuild();
        }

        // `Rc::clone` is a refcount bump: no copying of the row data, and no
        // allocation on the audio path.
        if input.is_just_pressed(Button::A) {
            self.player.play_sfx_shared(Rc::clone(&self.sfx));
        }
        if input.is_just_pressed(Button::Start) {
            self.player.play_song_shared(Rc::clone(&self.track));
        }
        if input.is_just_pressed(Button::Select) {
            self.player.stop();
        }

        self.player.frame();
    }
}

#[agb::entry]
fn main(mut _gba: agb::Gba) -> ! {
    let vblank = agb::interrupt::VBlank::get();
    let mut input = ButtonController::new();
    let mut tracker = Tracker::new();

    agb::println!("up/down: transpose, left/right: tempo");
    agb::println!("A: play sfx, Start: play song, Select: stop");

    loop {
        input.update();
        tracker.update(&input);
        vblank.wait_for_vblank();
    }
}
