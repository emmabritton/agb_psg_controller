use crate::{ChannelState, dirty, silence_state};
use agb::fixnum::Num;
use eb_agb_psg_interop::{
    Instrument, NOISE_NOTE_MAX, NOISE_TABLE, NOTE_MAX, NOTE_OFF, NOTE_PERIODS, NUM_CHANNELS,
    PatternSlot, PsgEffect, Sfx, SfxChannel, Track, WAVE_NOTE_OFFSET,
};

use crate::channels::square::{self, SquareChannel};
use crate::channels::{envelope_bits, noise, wave};
use crate::effects::{self, Jump, TimingChange};
use crate::registers::{MASTER_ENABLE, PSG_RATIO_MASK, SOUNDCNT_H, SOUNDCNT_L, SOUNDCNT_X};

/// How loud the PSG output is mixed relative to the DMA (agb mixer) output.
/// Use [`PsgRatio::Full`] when the PSG is the only sound source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PsgRatio {
    Quarter = 0,
    Half = 1,
    Full = 2,
}

/// A playing sound effect: its own row/tick machinery over one channel.
struct ActiveSfx<'a> {
    sfx: &'a Sfx,
    frame_acc: Num<u32, 8>,
    frames_per_tick: Num<u32, 8>,
    ticks_per_row: u32,
    tick: u32,
    row: u32,
    state: ChannelState,
}

fn sfx_hardware_channel(sfx: &Sfx) -> usize {
    match sfx.channel {
        SfxChannel::SquareSweep => 0,
        SfxChannel::Square => 1,
        SfxChannel::Wave => 2,
        SfxChannel::Noise => 3,
    }
}

/// An opaque snapshot of a song's playback position, obtained from
/// [`Player::position`] and later restored with [`Player::set_position`] to
/// resume playback (e.g. across a save/load). Only valid for the [`Track`] it
/// was taken from.
#[derive(Debug, Clone, Copy)]
pub struct PlaybackPosition {
    frame_acc: Num<u32, 8>,
    frames_per_tick: Num<u32, 8>,
    ticks_per_row: u32,
    tick: u32,
    row: u32,
    order_pos: u32,
}

/// The playback position of the current song.
struct MusicState<'track> {
    track: &'track Track,
    frame_acc: Num<u32, 8>,
    frames_per_tick: Num<u32, 8>,
    ticks_per_row: u32,
    tick: u32,
    row: u32,
    order_pos: u32,
    pending_jump: Option<(u32, u32)>,
    finished: bool,
}

impl<'track> MusicState<'track> {
    fn new(track: &'track Track) -> Self {
        Self {
            track,
            frame_acc: Num::new(0),
            frames_per_tick: track.frames_per_tick,
            ticks_per_row: track.ticks_per_row,
            tick: 0,
            row: 0,
            order_pos: 0,
            pending_jump: None,
            finished: false,
        }
    }

    fn current_pattern(&self) -> agb_psg_interop::Pattern {
        let pattern_index = self.track.pattern_order[self.order_pos as usize];
        self.track.patterns[pattern_index as usize]
    }

    fn tick(&mut self, channels: &mut [ChannelState; NUM_CHANNELS]) {
        if self.tick == 0 {
            self.process_row(channels);
        } else {
            self.process_effect_tick(channels);
        }
        self.tick += 1;
        if self.tick >= self.ticks_per_row {
            self.tick = 0;
            self.advance_row(channels);
        }
    }

    fn process_row(&mut self, channels: &mut [ChannelState; NUM_CHANNELS]) {
        let track = self.track;
        let pattern = self.current_pattern();
        let row_start = (pattern.start_row + self.row) as usize * NUM_CHANNELS;
        let mut timing = TimingChange::default();

        for (channel, state) in channels.iter_mut().enumerate() {
            let slot = track.pattern_data[row_start + channel];
            state.effect = slot.effect;
            state.delayed = None;

            if matches!(slot.effect, PsgEffect::NoteDelay(t) if t > 0) {
                state.delayed = Some(slot);
            } else {
                apply_cell(state, &track.instruments, &slot, channel);
            }
            effects::apply_row_effect(state, channel, &mut timing);
        }

        self.apply_timing(timing);
    }

    fn process_effect_tick(&mut self, channels: &mut [ChannelState; NUM_CHANNELS]) {
        let track = self.track;
        let tick = self.tick;
        for (channel, state) in channels.iter_mut().enumerate() {
            if let Some(slot) = state.delayed
                && matches!(slot.effect, PsgEffect::NoteDelay(t) if t as u32 == tick)
            {
                state.delayed = None;
                apply_cell(state, &track.instruments, &slot, channel);
            }
            effects::apply_tick_effect(state, channel, tick);
        }
    }

    fn apply_timing(&mut self, timing: TimingChange) {
        if let Some(ticks) = timing.ticks_per_row {
            self.ticks_per_row = ticks;
        }
        if let Some(frames) = timing.frames_per_tick {
            self.frames_per_tick = frames;
        }
        match timing.jump {
            Some(Jump::Position(target)) => self.pending_jump = Some((target as u32, 0)),
            Some(Jump::Break(row)) => self.pending_jump = Some((self.order_pos + 1, row as u32)),
            None => {}
        }
    }

    fn advance_row(&mut self, channels: &mut [ChannelState; NUM_CHANNELS]) {
        if let Some((order_pos, row)) = self.pending_jump.take() {
            if (order_pos as usize) < self.track.pattern_order.len() {
                self.order_pos = order_pos;
                let last_row = self.current_pattern().num_rows - 1;
                self.row = row.min(last_row);
            } else {
                self.end_of_song(channels);
            }
            return;
        }

        self.row += 1;
        if self.row < self.current_pattern().num_rows {
            return;
        }
        self.row = 0;
        self.order_pos += 1;
        if (self.order_pos as usize) < self.track.pattern_order.len() {
            return;
        }
        self.end_of_song(channels);
    }

    fn end_of_song(&mut self, channels: &mut [ChannelState; NUM_CHANNELS]) {
        match self.track.loop_order_index {
            Some(loop_to) => {
                self.order_pos = loop_to;
                self.row = 0;
            }
            None => {
                self.finished = true;
                for state in channels {
                    state.dirty = dirty::SILENCE;
                }
            }
        }
    }
}

/// Plays an optional [`Track`] on the PSG, plus one-shot [`Sfx`]s that
/// temporarily steal a channel from the music. Call [`Player::frame`] once per
/// frame.
///
/// For sound effects without music, create the player with
/// [`Player::sfx_only`]; a song can be started (or replaced) at any time with
/// [`Player::play_song`].
///
/// Uses no hardware timers and coexists with agb's mixer: the PSG mix level is
/// re-asserted every frame, so the mixer can be created before or after this.
pub struct Player<'track> {
    music: Option<MusicState<'track>>,
    psg_ratio: u16,
    /// Our copy of SOUNDCNT_L, so panning updates are a single write.
    pan_shadow: u16,
    channels: [ChannelState; NUM_CHANNELS],
    sfx: [Option<ActiveSfx<'track>>; NUM_CHANNELS],
}

impl<'track> Player<'track> {
    /// Creates a player that starts playing `track` immediately.
    pub fn new(track: &'track Track) -> Self {
        let mut player = Self::sfx_only();
        player.play_song(track);
        player
    }

    /// Creates a player with no music — useful when the PSG is only used for
    /// sound effects. Start a song later with [`Player::play_song`] if needed.
    pub fn sfx_only() -> Self {
        let player = Self {
            music: None,
            psg_ratio: PsgRatio::Full as u16,
            pan_shadow: 0xFF77,
            channels: Default::default(),
            sfx: [None, None, None, None],
        };
        // Master enable must be set first: all other PSG registers ignore
        // writes while it is clear.
        SOUNDCNT_X.set_bits(MASTER_ENABLE, MASTER_ENABLE);
        // Master L/R volume 7 and all four channels enabled both sides.
        SOUNDCNT_L.set(0xFF77);
        SOUNDCNT_H.set_bits(PSG_RATIO_MASK, player.psg_ratio);
        player
    }

    /// Starts playing a song from the beginning, replacing the current one if
    /// any. Playing sound effects keep their channels until they finish.
    pub fn play_song(&mut self, track: &'track Track) {
        self.music = Some(MusicState::new(track));
        for state in &mut self.channels {
            *state = ChannelState {
                dirty: dirty::SILENCE,
                ..ChannelState::default()
            };
        }
    }

    /// Returns a snapshot of the current song's playback position, or `None`
    /// if no song is playing. Save this to resume playback later with
    /// [`Player::set_position`].
    pub fn position(&self) -> Option<PlaybackPosition> {
        self.music.as_ref().map(|music| PlaybackPosition {
            frame_acc: music.frame_acc,
            frames_per_tick: music.frames_per_tick,
            ticks_per_row: music.ticks_per_row,
            tick: music.tick,
            row: music.row,
            order_pos: music.order_pos,
        })
    }

    /// Resumes playback of the current song at a previously saved
    /// [`PlaybackPosition`]. Has no effect if no song is playing, or if the
    /// position does not fit the current track (it must have been taken from
    /// the same one). Channels are reset as if the song had just been
    /// (re)started.
    pub fn set_position(&mut self, position: PlaybackPosition) {
        let Some(music) = &mut self.music else {
            return;
        };
        if position.order_pos as usize >= music.track.pattern_order.len() {
            return;
        }
        music.frame_acc = position.frame_acc;
        music.frames_per_tick = position.frames_per_tick;
        music.ticks_per_row = position.ticks_per_row;
        music.tick = position.tick;
        music.order_pos = position.order_pos;
        // Patterns differ in length, so a row from another track may not exist.
        music.row = position
            .row
            .min(music.current_pattern().num_rows.saturating_sub(1));
        music.pending_jump = None;
        music.finished = false;
        for state in &mut self.channels {
            *state = ChannelState {
                dirty: dirty::SILENCE,
                ..ChannelState::default()
            };
        }
    }

    /// Stops the music (playing sound effects continue).
    pub fn stop_song(&mut self) {
        self.music = None;
        for state in &mut self.channels {
            state.dirty = dirty::SILENCE;
        }
    }

    /// Sets the PSG mix level (defaults to [`PsgRatio::Full`]). Use a lower
    /// ratio when also using agb's software mixer.
    pub fn with_psg_ratio(mut self, ratio: PsgRatio) -> Self {
        self.psg_ratio = ratio as u16;
        SOUNDCNT_H.set_bits(PSG_RATIO_MASK, self.psg_ratio);
        self
    }

    /// Advances playback by one frame. Call exactly once per frame, e.g.
    /// right before or after `mixer.frame()`.
    pub fn frame(&mut self) {
        // agb's mixer rewrites SOUNDCNT_H (zeroing the PSG ratio bits) when it
        // is created, so re-assert our bits every frame to be order-independent.
        SOUNDCNT_X.set_bits(MASTER_ENABLE, MASTER_ENABLE);
        SOUNDCNT_H.set_bits(PSG_RATIO_MASK, self.psg_ratio);

        if let Some(music) = &mut self.music
            && !music.finished
        {
            music.frame_acc += Num::new(1);
            while music.frame_acc >= music.frames_per_tick {
                music.frame_acc -= music.frames_per_tick;
                music.tick(&mut self.channels);
                if music.finished {
                    break;
                }
            }
        }

        self.step_sfx();
        self.realise();
    }

    /// Starts a sound effect, stealing its channel from the music. The music
    /// keeps advancing silently on that channel and becomes audible again at
    /// its next note after the effect ends. A new effect on the same channel
    /// replaces the current one; effects on different channels play together.
    pub fn play_sfx(&mut self, sfx: &'track Sfx) {
        let channel = sfx_hardware_channel(sfx);
        self.sfx[channel] = Some(ActiveSfx {
            sfx,
            frame_acc: Num::new(0),
            frames_per_tick: sfx.frames_per_tick,
            ticks_per_row: sfx.ticks_per_row,
            tick: 0,
            row: 0,
            state: ChannelState::default(),
        });
    }

    /// Stops all playing sound effects, returning their channels to the music.
    pub fn stop_sfx(&mut self) {
        for channel in 0..NUM_CHANNELS {
            if self.sfx[channel].is_some() {
                self.release_sfx(channel);
            }
        }
    }

    /// Stops everything: sound effects and music, silencing all channels.
    pub fn stop(&mut self) {
        self.stop_sfx();
        self.stop_song();
        self.realise();
    }

    /// True when there is nothing left to play: no song, or a non-looping song
    /// that has ended. Sound effects do not affect this.
    pub fn is_finished(&self) -> bool {
        self.music.as_ref().is_none_or(|music| music.finished)
    }

    /// (order position, row, tick) — for tests and debugging. (0, 0, 0) with
    /// no music.
    #[doc(hidden)]
    pub fn debug_position(&self) -> (u32, u32, u32) {
        self.music
            .as_ref()
            .map_or((0, 0, 0), |music| (music.order_pos, music.row, music.tick))
    }

    /// (period, volume) of a music channel — for tests and debugging.
    #[doc(hidden)]
    pub fn debug_channel(&self, channel: usize) -> (u16, u8) {
        (self.channels[channel].period, self.channels[channel].volume)
    }

    /// Whether a sound effect currently owns this channel — for tests.
    #[doc(hidden)]
    pub fn debug_sfx_active(&self, channel: usize) -> bool {
        self.sfx[channel].is_some()
    }

    /// Advances every active sound effect by one frame, releasing finished ones.
    fn step_sfx(&mut self) {
        for channel in 0..NUM_CHANNELS {
            let Some(active) = &mut self.sfx[channel] else {
                continue;
            };
            active.frame_acc += Num::new(1);
            let mut ended = false;
            while active.frame_acc >= active.frames_per_tick {
                active.frame_acc -= active.frames_per_tick;
                if active.tick == 0 {
                    let slot = active.sfx.rows[active.row as usize];
                    active.state.effect = slot.effect;
                    active.state.delayed = None;
                    if matches!(slot.effect, PsgEffect::NoteDelay(t) if t > 0) {
                        active.state.delayed = Some(slot);
                    } else {
                        apply_cell(&mut active.state, &active.sfx.instruments, &slot, channel);
                    }
                    let mut timing = TimingChange::default();
                    effects::apply_row_effect(&mut active.state, channel, &mut timing);
                    if let Some(ticks) = timing.ticks_per_row {
                        active.ticks_per_row = ticks;
                    }
                    if let Some(frames) = timing.frames_per_tick {
                        active.frames_per_tick = frames;
                    }
                } else {
                    if let Some(slot) = active.state.delayed
                        && matches!(slot.effect, PsgEffect::NoteDelay(t) if t as u32 == active.tick)
                    {
                        active.state.delayed = None;
                        apply_cell(&mut active.state, &active.sfx.instruments, &slot, channel);
                    }
                    effects::apply_tick_effect(&mut active.state, channel, active.tick);
                }
                active.tick += 1;
                if active.tick >= active.ticks_per_row {
                    active.tick = 0;
                    active.row += 1;
                    if active.row as usize >= active.sfx.rows.len() {
                        ended = true;
                        break;
                    }
                }
            }
            if ended {
                self.release_sfx(channel);
            }
        }
    }

    /// Ends a sound effect: silence the channel and hand it back to the music,
    /// which stays silent until its next note-on (documented resync policy —
    /// retriggering a mid-decay note would sound worse than a short gap).
    fn release_sfx(&mut self, channel: usize) {
        self.sfx[channel] = None;
        silence_hardware(channel);
        let state = &mut self.channels[channel];
        state.dirty = 0;
        if channel == 2 {
            // The effect overwrote wave RAM; force a re-upload at the next note.
            state.loaded_wave = None;
        }
    }

    fn realise(&mut self) {
        let mut pan_changed = false;
        // Single pass over 0..NUM_CHANNELS (rather than two separate passes)
        // — same work, half the loop overhead, since this runs every frame.
        for channel in 0..NUM_CHANNELS {
            if let Some(active) = &mut self.sfx[channel] {
                // Music state keeps advancing while stolen, but its hardware
                // writes are dropped. Panning intent survives in the state.
                pan_changed |= self.channels[channel].dirty & dirty::PAN != 0;
                self.channels[channel].dirty = 0;
                let sfx = active.sfx;
                flush_state(
                    &mut active.state,
                    channel,
                    &sfx.instruments,
                    &sfx.wave_tables,
                );
                continue;
            }
            let state = &mut self.channels[channel];
            pan_changed |= match &self.music {
                Some(music) => flush_state(
                    state,
                    channel,
                    &music.track.instruments,
                    &music.track.wave_tables,
                ),
                // No song: the only meaningful music-channel flags are
                // silences (e.g. from stop_song).
                None => flush_state(state, channel, &[], &[]),
            };
        }
        if pan_changed {
            self.write_panning();
        }
    }

    /// Rebuilds SOUNDCNT_L from the per-channel pan flags (master volume 7/7).
    fn write_panning(&mut self) {
        let mut value = 0x0077;
        for (channel, state) in self.channels.iter().enumerate() {
            value |= (state.pan_right as u16) << (8 + channel);
            value |= (state.pan_left as u16) << (12 + channel);
        }
        if value != self.pan_shadow {
            self.pan_shadow = value;
            SOUNDCNT_L.set(value);
        }
    }
}

/// Applies a cell's instrument and note to a channel state.
fn apply_cell(
    state: &mut ChannelState,
    instruments: &[Instrument],
    slot: &PatternSlot,
    channel: usize,
) {
    if slot.instrument != 0 {
        state.instrument = slot.instrument;
        if let Some(instrument) = instruments.get(slot.instrument as usize - 1) {
            match instrument {
                Instrument::Square { duty, envelope, .. } => {
                    state.volume = envelope.initial_volume;
                    state.duty = *duty;
                }
                Instrument::Wave { volume, .. } => state.volume = *volume,
                Instrument::Noise { envelope, .. } => state.volume = envelope.initial_volume,
            }
        }
    }

    match slot.note {
        0 => {}
        NOTE_OFF => silence_state(state),
        note if note <= NOTE_MAX => {
            state.note = note;
            state.noise_note = note;
            let period = period_for(note, channel);
            if matches!(slot.effect, PsgEffect::TonePortamento(_)) {
                // Slide from the current pitch instead of retriggering.
                state.target_period = period;
            } else {
                state.period = period;
                state.target_period = period;
                state.vibrato_phase = 0;
                state.vib_offset = 0;
                // A retrigger supersedes any pending silence (e.g. from a song
                // switch); without this the new song's first notes are eaten.
                state.dirty = (state.dirty & !dirty::SILENCE) | dirty::RETRIGGER;
            }
        }
        _ => {}
    }
}

fn period_for(note: u8, channel: usize) -> u16 {
    if channel == 3 {
        // Noise pitch comes from NOISE_TABLE at realise time.
        return 0;
    }
    let table_note = if channel == 2 {
        note + WAVE_NOTE_OFFSET
    } else {
        note
    };
    NOTE_PERIODS[(table_note - 1) as usize]
}

/// Writes a channel state's pending changes to hardware. Returns whether the
/// panning registers need refreshing.
fn flush_state(
    state: &mut ChannelState,
    channel: usize,
    instruments: &[Instrument],
    wave_tables: &[[u8; 16]],
) -> bool {
    if state.dirty == 0 {
        return false;
    }
    let flags = state.dirty;
    state.dirty = 0;
    let pan_changed = flags & dirty::PAN != 0;

    if flags & dirty::SILENCE != 0 {
        silence_hardware(channel);
        return pan_changed;
    }

    let Some(&instrument) = state
        .instrument
        .checked_sub(1)
        .and_then(|i| instruments.get(i as usize))
    else {
        return pan_changed;
    };

    if flags & dirty::RETRIGGER != 0 {
        trigger_hardware(state, channel, instrument, wave_tables);
    } else {
        if flags & dirty::PERIOD != 0 {
            write_period(state, channel, instrument);
        }
        if flags & dirty::VOLUME != 0 {
            debug_assert!(channel == 2, "only the wave channel sets dirty::VOLUME");
            wave::set_volume(state.volume);
        }
    }
    pan_changed
}

fn silence_hardware(channel: usize) {
    match channel {
        0 => square::silence(SquareChannel::One),
        1 => square::silence(SquareChannel::Two),
        2 => wave::silence(),
        _ => noise::silence(),
    }
}

/// The hardware period for a channel state right now: the slid period plus the
/// vibrato offset.
fn effective_period(state: &ChannelState) -> u16 {
    (state.period as i32 + state.vib_offset as i32).clamp(0, 2047) as u16
}

fn trigger_hardware(
    state: &mut ChannelState,
    channel: usize,
    instrument: Instrument,
    wave_tables: &[[u8; 16]],
) {
    match (channel, instrument) {
        (
            0 | 1,
            Instrument::Square {
                envelope,
                sweep,
                length,
                ..
            },
        ) => {
            let square_channel = if channel == 0 {
                SquareChannel::One
            } else {
                SquareChannel::Two
            };
            if channel == 0 {
                let sweep_bits = sweep.map_or(0, |s| {
                    ((s.time as u16) << 4) | ((s.decreasing as u16) << 3) | s.shift as u16
                });
                square::set_sweep(sweep_bits);
            }
            let envelope = envelope_with_volume(&envelope, state.volume);
            square::trigger(
                square_channel,
                state.duty,
                envelope,
                length,
                effective_period(state),
            );
        }
        (2, Instrument::Wave { wave_table, .. }) => {
            if state.loaded_wave != Some(wave_table) {
                let Some(table) = wave_tables.get(wave_table as usize) else {
                    return;
                };
                wave::upload_wave(table);
                state.loaded_wave = Some(wave_table);
            }
            wave::trigger(state.volume, effective_period(state));
        }
        (
            3,
            Instrument::Noise {
                envelope,
                short_lfsr,
            },
        ) => {
            noise::trigger(
                envelope_with_volume(&envelope, state.volume),
                noise_poly(state),
                short_lfsr,
            );
        }
        // Parser validation makes instrument/channel mismatches unreachable
        // for macro-built tracks; ignore rather than panic on bad input.
        _ => {}
    }
}

fn write_period(state: &ChannelState, channel: usize, instrument: Instrument) {
    match channel {
        0 => square::set_period(SquareChannel::One, effective_period(state)),
        1 => square::set_period(SquareChannel::Two, effective_period(state)),
        2 => wave::set_period(effective_period(state)),
        _ => {
            if let Instrument::Noise { short_lfsr, .. } = instrument {
                noise::set_poly(noise_poly(state), short_lfsr);
            }
        }
    }
}

/// The `SOUND4CNT_H` polynomial byte for the channel's current noise pitch.
/// Clamped rather than indexed raw: [`Track`] is public, so a hand-built one
/// can hold a note outside the table.
fn noise_poly(state: &ChannelState) -> u8 {
    let index = state.noise_note.clamp(1, NOISE_NOTE_MAX) - 1;
    NOISE_TABLE[index as usize]
}

/// NRx2 envelope byte with the channel's current volume in place of the
/// instrument's initial volume.
fn envelope_with_volume(envelope: &agb_psg_interop::EnvelopeSpec, volume: u8) -> u16 {
    let mut spec = *envelope;
    spec.initial_volume = volume;
    envelope_bits(&spec)
}
