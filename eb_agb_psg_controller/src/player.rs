use crate::{ChannelState, dirty, silence_state};
use agb_fixnum::num;
#[cfg(feature = "dynamic")]
use alloc::rc::Rc;
use eb_agb_psg_interop::{
    FrameCount, Instrument, NOISE_NOTE_MAX, NOISE_TABLE, NOTE_MAX, NOTE_OFF, NOTE_PERIODS,
    NUM_CHANNELS, PatternSlot, PsgEffect, Sfx, SfxChannel, Track, WAVE_NOTE_OFFSET,
};

use crate::channels::square::{self, SquareChannel};
use crate::channels::{envelope_bits, noise, wave};
use crate::effects::{self, Jump, TimingChange};
use crate::registers::{MASTER_ENABLE, PSG_RATIO_MASK, SOUNDCNT_H, SOUNDCNT_L, SOUNDCNT_X};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PsgRatio {
    Quarter = 0,
    Half = 1,
    Full = 2,
}

#[cfg(not(feature = "dynamic"))]
struct Held<'a, T>(&'a T);

#[cfg(feature = "dynamic")]
enum Held<'a, T> {
    Ref(&'a T),
    Shared(Rc<T>),
}

#[cfg(not(feature = "dynamic"))]
impl<'a, T> Held<'a, T> {
    fn borrowed(value: &'a T) -> Self {
        Held(value)
    }
}

#[cfg(feature = "dynamic")]
impl<'a, T> Held<'a, T> {
    fn borrowed(value: &'a T) -> Self {
        Held::Ref(value)
    }
}

#[cfg(not(feature = "dynamic"))]
impl<T> core::ops::Deref for Held<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.0
    }
}

#[cfg(feature = "dynamic")]
impl<T> core::ops::Deref for Held<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        match self {
            Held::Ref(value) => value,
            Held::Shared(value) => value,
        }
    }
}

struct ActiveSfx<'a> {
    sfx: Held<'a, Sfx>,
    frame_acc: FrameCount,
    frames_per_tick: FrameCount,
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

//opaque position
#[derive(Debug, Clone, Copy)]
pub struct PlaybackPosition {
    frame_acc: FrameCount,
    frames_per_tick: FrameCount,
    ticks_per_row: u32,
    tick: u32,
    row: u32,
    order_pos: u32,
}

/// The playback position of the current song.
struct MusicState<'track> {
    track: Held<'track, Track>,
    frame_acc: FrameCount,
    frames_per_tick: FrameCount,
    ticks_per_row: u32,
    tick: u32,
    row: u32,
    order_pos: u32,
    pending_jump: Option<(u32, u32)>,
    finished: bool,
    /// Where to loop back to at the end, seeded from the track and overridable
    /// per playback by [`Player::set_looping`]; `None` = stop instead.
    loop_to: Option<u32>,
}

impl<'track> MusicState<'track> {
    fn new(track: Held<'track, Track>) -> Self {
        Self {
            frame_acc: num!(0),
            frames_per_tick: track.frames_per_tick,
            ticks_per_row: track.ticks_per_row,
            tick: 0,
            row: 0,
            order_pos: 0,
            pending_jump: None,
            finished: false,
            loop_to: track.loop_order_index,
            // Last: the two reads above borrow `track` before it moves.
            track,
        }
    }

    fn current_pattern(&self) -> eb_agb_psg_interop::Pattern {
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
        let track = &*self.track;
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
        let track = &*self.track;
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
        match self.loop_to {
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

pub struct Player<'track> {
    music: Option<MusicState<'track>>,
    psg_ratio: u16,
    pan_shadow: u16,
    channels: [ChannelState; NUM_CHANNELS],
    sfx: [Option<ActiveSfx<'track>>; NUM_CHANNELS],
}

impl<'track> Player<'track> {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let player = Self {
            music: None,
            psg_ratio: PsgRatio::Full as u16,
            pan_shadow: 0xFF77,
            channels: Default::default(),
            sfx: [None, None, None, None],
        };
        SOUNDCNT_X.set_bits(MASTER_ENABLE, MASTER_ENABLE);
        SOUNDCNT_L.set(0xFF77);
        SOUNDCNT_H.set_bits(PSG_RATIO_MASK, player.psg_ratio);
        player
    }

    pub fn play_song(&mut self, track: &'track Track) {
        self.start_song(Held::borrowed(track));
    }

    #[cfg(feature = "dynamic")]
    pub fn play_song_shared(&mut self, track: Rc<Track>) {
        self.start_song(Held::Shared(track));
    }

    fn start_song(&mut self, track: Held<'track, Track>) {
        self.music = Some(MusicState::new(track));
        for state in &mut self.channels {
            *state = ChannelState {
                dirty: dirty::SILENCE,
                ..ChannelState::default()
            };
        }
        self.write_panning();
    }

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
        self.write_panning();
    }

    pub fn set_looping(&mut self, looping: bool) {
        if let Some(music) = &mut self.music {
            music.loop_to = if looping {
                music.track.loop_order_index
            } else {
                None
            };
        }
    }

    pub fn stop_song(&mut self) {
        self.music = None;
        for state in &mut self.channels {
            state.dirty = dirty::SILENCE;
        }
    }

    pub fn with_psg_ratio(mut self, ratio: PsgRatio) -> Self {
        self.psg_ratio = ratio as u16;
        SOUNDCNT_H.set_bits(PSG_RATIO_MASK, self.psg_ratio);
        self
    }

    pub fn frame(&mut self) {
        //agb zeroes these, so reset
        SOUNDCNT_X.set_bits(MASTER_ENABLE, MASTER_ENABLE);
        SOUNDCNT_H.set_bits(PSG_RATIO_MASK, self.psg_ratio);

        if let Some(music) = &mut self.music
            && !music.finished
        {
            music.frame_acc += num!(1);
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

    pub fn play_sfx(&mut self, sfx: &'track Sfx) {
        self.start_sfx(Held::borrowed(sfx));
    }

    #[cfg(feature = "dynamic")]
    pub fn play_sfx_shared(&mut self, sfx: Rc<Sfx>) {
        self.start_sfx(Held::Shared(sfx));
    }

    fn start_sfx(&mut self, sfx: Held<'track, Sfx>) {
        let channel = sfx_hardware_channel(&sfx);
        self.sfx[channel] = Some(ActiveSfx {
            frame_acc: num!(0),
            frames_per_tick: sfx.frames_per_tick,
            ticks_per_row: sfx.ticks_per_row,
            tick: 0,
            row: 0,
            state: ChannelState::default(),
            // Last: the two reads above borrow `sfx` before it moves.
            sfx,
        });
    }

    pub fn stop_sfx(&mut self) {
        for channel in 0..NUM_CHANNELS {
            if self.sfx[channel].is_some() {
                self.release_sfx(channel);
            }
        }
    }

    pub fn stop(&mut self) {
        self.stop_sfx();
        self.stop_song();
        self.realise();
    }

    pub fn is_finished(&self) -> bool {
        self.music.as_ref().is_none_or(|music| music.finished)
    }

    #[doc(hidden)]
    pub fn debug_position(&self) -> (u32, u32, u32) {
        self.music
            .as_ref()
            .map_or((0, 0, 0), |music| (music.order_pos, music.row, music.tick))
    }

    #[doc(hidden)]
    pub fn debug_channel(&self, channel: usize) -> (u16, u8) {
        (self.channels[channel].period, self.channels[channel].volume)
    }

    #[doc(hidden)]
    pub fn debug_sfx_active(&self, channel: usize) -> bool {
        self.sfx[channel].is_some()
    }

    fn step_sfx(&mut self) {
        for channel in 0..NUM_CHANNELS {
            let Some(active) = &mut self.sfx[channel] else {
                continue;
            };
            active.frame_acc += num!(1);
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

    fn release_sfx(&mut self, channel: usize) {
        self.sfx[channel] = None;
        silence_hardware(channel);
        let state = &mut self.channels[channel];
        state.dirty = 0;
        if channel == 2 {
            state.loaded_wave = None;
        }
    }

    fn realise(&mut self) {
        let mut pan_changed = false;
        for channel in 0..NUM_CHANNELS {
            if let Some(active) = &mut self.sfx[channel] {
                pan_changed |= self.channels[channel].dirty & dirty::PAN != 0;
                self.channels[channel].dirty = 0;
                let ActiveSfx { sfx, state, .. } = active;
                flush_state(state, channel, &sfx.instruments, &sfx.wave_tables);
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
                None => flush_state(state, channel, &[], &[]),
            };
        }
        if pan_changed {
            self.write_panning();
        }
    }

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
                state.target_period = period;
            } else {
                state.period = period;
                state.target_period = period;
                state.vibrato_phase = 0;
                state.vib_offset = 0;
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
        _ => {}
    }
}

fn write_period(state: &ChannelState, channel: usize, instrument: Instrument) {
    let length_enabled = matches!(
        instrument,
        Instrument::Square {
            length: Some(_),
            ..
        }
    );
    match channel {
        0 => square::set_period(SquareChannel::One, effective_period(state), length_enabled),
        1 => square::set_period(SquareChannel::Two, effective_period(state), length_enabled),
        2 => wave::set_period(effective_period(state)),
        _ => {
            if let Instrument::Noise { short_lfsr, .. } = instrument {
                noise::set_poly(noise_poly(state), short_lfsr);
            }
        }
    }
}

fn noise_poly(state: &ChannelState) -> u8 {
    let index = state.noise_note.clamp(1, NOISE_NOTE_MAX) - 1;
    NOISE_TABLE[index as usize]
}

fn envelope_with_volume(envelope: &eb_agb_psg_interop::EnvelopeSpec, volume: u8) -> u16 {
    let mut spec = *envelope;
    spec.initial_volume = volume;
    envelope_bits(&spec)
}
