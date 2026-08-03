use agb::fixnum::{Num, num};
use agb::rng::RandomNumberGenerator;
use alloc::borrow::Cow;
use alloc::vec;
use alloc::vec::Vec;
use eb_agb_psg_controller::*;

pub const FRAMES_PER_TICK_UI_MAX: u32 = 30;

#[derive(Debug)]
pub struct SfxDocument {
    sfx: Sfx,
    rows_gen: u16,
}

impl SfxDocument {
    pub fn new() -> Self {
        Self {
            sfx: Sfx {
                channel: SfxChannel::Square,
                instruments: Cow::Owned(Vec::new()),
                wave_tables: Cow::Owned(Vec::new()),
                rows: Cow::Owned(vec![PatternSlot::EMPTY]),
                frames_per_tick: num!(2),
                ticks_per_row: 4,
            },
            rows_gen: 0,
        }
    }

    pub fn from_sfx(sfx: Sfx) -> Self {
        let mut rows = sfx.rows.into_owned();
        if rows.is_empty() {
            rows.push(PatternSlot::EMPTY);
        }
        Self {
            sfx: Sfx {
                channel: sfx.channel,
                instruments: Cow::Owned(sfx.instruments.into_owned()),
                wave_tables: Cow::Owned(sfx.wave_tables.into_owned()),
                rows: Cow::Owned(rows),
                frames_per_tick: sfx.frames_per_tick,
                ticks_per_row: sfx.ticks_per_row,
            },
            rows_gen: 0,
        }
    }

    pub fn channel(&self) -> SfxChannel {
        self.sfx.channel
    }

    pub fn rows(&self) -> &[PatternSlot] {
        &self.sfx.rows
    }

    pub fn instruments(&self) -> &[Instrument] {
        &self.sfx.instruments
    }

    pub fn wave_tables(&self) -> &[[u8; 16]] {
        &self.sfx.wave_tables
    }

    pub fn frames_per_tick(&self) -> Num<u32, 8> {
        self.sfx.frames_per_tick
    }

    pub fn ticks_per_row(&self) -> u32 {
        self.sfx.ticks_per_row
    }

    pub fn rows_generation(&self) -> u16 {
        self.rows_gen
    }

    pub fn build_sfx(&self) -> Sfx {
        self.sfx.clone()
    }

    fn rows_mut(&mut self) -> &mut Vec<PatternSlot> {
        self.sfx.rows.to_mut()
    }

    fn instruments_mut(&mut self) -> &mut Vec<Instrument> {
        self.sfx.instruments.to_mut()
    }

    fn wave_tables_mut(&mut self) -> &mut Vec<[u8; 16]> {
        self.sfx.wave_tables.to_mut()
    }
}

impl SfxDocument {
    fn from_parts(
        channel: SfxChannel,
        instruments: Vec<Instrument>,
        mut wave_tables: Vec<[u8; 16]>,
        mut rows: Vec<PatternSlot>,
        frames_per_tick: Num<u32, 8>,
        ticks_per_row: u32,
    ) -> Self {
        wave_tables.truncate(limits::MAX_WAVE_TABLES);
        let mut doc = Self::new();
        doc.sfx.channel = channel;
        doc.sfx.wave_tables = Cow::Owned(wave_tables);
        doc.set_frames_per_tick(frames_per_tick);
        doc.set_ticks_per_row(ticks_per_row);
        for instrument in instruments {
            let added = doc.add_instrument(instrument);
            debug_assert!(added.is_some(), "preset built an invalid instrument");
        }
        if !rows.is_empty() {
            rows.truncate(limits::MAX_ROWS);
            doc.sfx.rows = Cow::Owned(rows);
        }
        doc.sanitize_rows();
        doc
    }

    fn square_doc(
        instrument: Instrument,
        mut rows: Vec<PatternSlot>,
        frames_per_tick: Num<u32, 8>,
        ticks_per_row: u32,
    ) -> Self {
        let channel = match instrument {
            Instrument::Square { sweep: Some(_), .. } => SfxChannel::SquareSweep,
            _ => SfxChannel::Square,
        };
        rows.push(note_slot(NOTE_OFF, 0));
        Self::from_parts(
            channel,
            vec![instrument],
            Vec::new(),
            rows,
            frames_per_tick,
            ticks_per_row,
        )
    }

    fn noise_doc(
        envelope: EnvelopeSpec,
        short_lfsr: bool,
        mut rows: Vec<PatternSlot>,
        frames_per_tick: Num<u32, 8>,
        ticks_per_row: u32,
    ) -> Self {
        rows.push(note_slot(NOTE_OFF, 0));
        Self::from_parts(
            SfxChannel::Noise,
            vec![Instrument::Noise {
                envelope,
                short_lfsr,
            }],
            Vec::new(),
            rows,
            frames_per_tick,
            ticks_per_row,
        )
    }

    pub fn random_powerup(rng: &mut RandomNumberGenerator) -> Self {
        let base = rand_u8(rng, 40, 52);
        let mut rows = vec![note_slot(base, 1)];
        if rand_bool(rng) {
            let mut note = base;
            for _ in 0..rand_u8(rng, 3, 5) {
                note = note.saturating_add(rand_u8(rng, 2, 5));
                rows.push(note_slot(note, 0));
            }
        } else {
            let rate = rand_u8(rng, 0x18, 0x48);
            push_fx_run(&mut rows, PsgEffect::PortamentoUp(rate), rand_u8(rng, 3, 5));
        }
        let envelope = rand_envelope(rng, 12, 15, 3, 5);
        Self::square_doc(
            square_instrument(rng, envelope, None, None),
            rows,
            Num::new(rand_u8(rng, 1, 2) as u32),
            rand_u8(rng, 2, 4) as u32,
        )
    }

    pub fn random_click(rng: &mut RandomNumberGenerator) -> Self {
        Self::noise_doc(
            rand_envelope(rng, 8, 11, 1, 1),
            false,
            vec![PatternSlot {
                note: rand_u8(rng, 25, 40),
                instrument: 1,
                effect: PsgEffect::NoteCut(rand_u8(rng, 1, 2)),
            }],
            num!(1),
            2,
        )
    }

    pub fn random_positive(rng: &mut RandomNumberGenerator) -> Self {
        let base = rand_u8(rng, 49, 58);
        let mut rows = vec![
            note_slot(base, 1),
            note_slot(base + 4, 0),
            note_slot(base + 7, 0),
        ];
        if rand_bool(rng) {
            rows.push(note_slot(base + 12, 0));
        }
        let envelope = rand_envelope(rng, 11, 14, 2, 3);
        Self::square_doc(
            square_instrument(rng, envelope, None, None),
            rows,
            num!(1),
            rand_u8(rng, 2, 3) as u32,
        )
    }

    pub fn random_negative(rng: &mut RandomNumberGenerator) -> Self {
        Self::noise_doc(
            rand_envelope(rng, 6, 10, 1, 1),
            rand_bool(rng),
            vec![note_slot(rand_u8(rng, 45, 60), 1)],
            num!(1),
            rand_u8(rng, 1, 2) as u32,
        )
    }

    pub fn random_shoot(rng: &mut RandomNumberGenerator) -> Self {
        let envelope = rand_envelope(rng, 12, 15, 1, 2);
        let sweep = SweepSpec {
            time: rand_u8(rng, 1, 2),
            decreasing: true,
            shift: rand_u8(rng, 3, 6),
        };
        let length = rand_u8(rng, 12, 32);
        Self::square_doc(
            square_instrument(rng, envelope, Some(sweep), Some(length)),
            vec![note_slot(rand_u8(rng, 55, 75), 1), PatternSlot::EMPTY],
            num!(1),
            rand_u8(rng, 2, 3) as u32,
        )
    }

    pub fn random_explosion(rng: &mut RandomNumberGenerator) -> Self {
        let mut rows = vec![
            note_slot(rand_u8(rng, 42, 52), 1),
            note_slot(rand_u8(rng, 26, 36), 0),
        ];
        let slide = rand_range(rng, -3, -2) as i8;
        push_fx_run(&mut rows, PsgEffect::VolumeSlide(slide), rand_u8(rng, 2, 3));
        Self::noise_doc(
            rand_envelope(rng, 14, 15, 5, 7),
            rand_u8(rng, 0, 3) == 0,
            rows,
            Num::new(rand_u8(rng, 2, 4) as u32),
            rand_u8(rng, 4, 6) as u32,
        )
    }

    pub fn random_beep(rng: &mut RandomNumberGenerator) -> Self {
        let envelope = rand_envelope(rng, 10, 14, 0, 0);
        Self::square_doc(
            square_instrument(rng, envelope, None, None),
            vec![note_slot(rand_u8(rng, 40, 70), 1)],
            Num::new(rand_u8(rng, 1, 2) as u32),
            rand_u8(rng, 3, 6) as u32,
        )
    }

    pub fn random_jump(rng: &mut RandomNumberGenerator) -> Self {
        let mut rows = vec![note_slot(rand_u8(rng, 28, 40), 1)];
        let rate = rand_u8(rng, 0x06, 0x14);
        push_fx_run(&mut rows, PsgEffect::PortamentoUp(rate), rand_u8(rng, 3, 4));
        let envelope = rand_envelope(rng, 10, 13, 4, 6);
        Self::square_doc(
            square_instrument(rng, envelope, None, None),
            rows,
            Num::new(rand_u8(rng, 1, 2) as u32),
            rand_u8(rng, 2, 3) as u32,
        )
    }

    pub fn random_hurt(rng: &mut RandomNumberGenerator, channel: Option<SfxChannel>) -> Self {
        let square = match channel {
            Some(SfxChannel::Noise) => false,
            Some(_) => true,
            None => rand_bool(rng),
        };
        if square {
            let mut rows = vec![note_slot(rand_u8(rng, 38, 50), 1)];
            let rate = rand_u8(rng, 0x20, 0x50);
            push_fx_run(&mut rows, PsgEffect::PortamentoDown(rate), 2);
            let envelope = rand_envelope(rng, 11, 14, 2, 3);
            Self::square_doc(
                square_instrument(rng, envelope, None, None),
                rows,
                Num::new(rand_u8(rng, 1, 2) as u32),
                2,
            )
        } else {
            let mut rows = vec![note_slot(rand_u8(rng, 30, 45), 1)];
            rows.push(fx_slot(PsgEffect::VolumeSlide(
                rand_range(rng, -4, -2) as i8
            )));
            Self::noise_doc(
                rand_envelope(rng, 10, 13, 2, 3),
                rand_bool(rng),
                rows,
                num!(1),
                rand_u8(rng, 2, 3) as u32,
            )
        }
    }

    pub fn random_laser(rng: &mut RandomNumberGenerator) -> Self {
        let mut rows = vec![note_slot(rand_u8(rng, 70, 85), 1)];
        let rate = rand_u8(rng, 0x30, 0x60);
        push_fx_run(
            &mut rows,
            PsgEffect::PortamentoDown(rate),
            rand_u8(rng, 2, 3),
        );
        Self::square_doc(
            Instrument::Square {
                duty: rand_u8(rng, 0, 1),
                envelope: rand_envelope(rng, 12, 15, 2, 4),
                sweep: None,
                length: None,
            },
            rows,
            num!(1),
            2,
        )
    }

    pub fn random_pickup(rng: &mut RandomNumberGenerator) -> Self {
        let base = rand_u8(rng, 55, 68);
        let envelope = rand_envelope(rng, 10, 14, 3, 4);
        Self::square_doc(
            square_instrument(rng, envelope, None, None),
            vec![
                note_slot(base, 1),
                note_slot(base + rand_u8(rng, 5, 9), 0),
                PatternSlot::EMPTY,
            ],
            num!(1),
            2,
        )
    }

    pub fn random_alarm(rng: &mut RandomNumberGenerator) -> Self {
        let low = rand_u8(rng, 50, 60);
        let high = low + rand_u8(rng, 3, 7);
        let mut rows = Vec::new();
        for _ in 0..rand_u8(rng, 2, 3) {
            rows.push(note_slot(low, 1));
            rows.push(note_slot(high, 0));
        }
        let envelope = rand_envelope(rng, 11, 14, 0, 0);
        Self::square_doc(
            square_instrument(rng, envelope, None, None),
            rows,
            Num::new(rand_u8(rng, 2, 3) as u32),
            rand_u8(rng, 4, 6) as u32,
        )
    }

    pub fn random_teleport(rng: &mut RandomNumberGenerator) -> Self {
        let vibrato = PsgEffect::Vibrato {
            speed: rand_u8(rng, 8, 12),
            depth: rand_u8(rng, 6, 12),
        };
        let drift_rate = rand_u8(rng, 0x10, 0x30);
        let drift = if rand_bool(rng) {
            PsgEffect::PortamentoUp(drift_rate)
        } else {
            PsgEffect::PortamentoDown(drift_rate)
        };
        Self::from_parts(
            SfxChannel::Wave,
            vec![Instrument::Wave {
                wave_table: 0,
                volume: rand_u8(rng, 3, 4),
            }],
            vec![rand_wave_table(rng)],
            vec![
                note_slot(rand_u8(rng, 30, 50), 1),
                fx_slot(vibrato),
                fx_slot(drift),
                fx_slot(vibrato),
                note_slot(NOTE_OFF, 0),
            ],
            Num::new(rand_u8(rng, 2, 3) as u32),
            rand_u8(rng, 3, 4) as u32,
        )
    }

    pub fn random_footstep(rng: &mut RandomNumberGenerator) -> Self {
        Self::noise_doc(
            rand_envelope(rng, 7, 9, 1, 1),
            rand_bool(rng),
            vec![note_slot(rand_u8(rng, 30, 40), 1)],
            num!(1),
            rand_u8(rng, 4, 6) as u32,
        )
    }

    pub fn random_magic(rng: &mut RandomNumberGenerator) -> Self {
        let arpeggio = PsgEffect::Arpeggio(rand_u8(rng, 3, 5), rand_u8(rng, 7, 11));
        let mut rows = vec![PatternSlot {
            note: rand_u8(rng, 60, 75),
            instrument: 1,
            effect: arpeggio,
        }];
        push_fx_run(&mut rows, arpeggio, rand_u8(rng, 2, 4));
        let envelope = rand_envelope(rng, 11, 14, 3, 5);
        Self::square_doc(
            square_instrument(rng, envelope, None, None),
            rows,
            num!(1),
            rand_u8(rng, 2, 3) as u32,
        )
    }
}

impl SfxDocument {
    pub fn set_channel(&mut self, channel: SfxChannel) {
        self.sfx.channel = channel;
        self.sanitize_rows();
    }

    pub fn set_ticks_per_row(&mut self, ticks: u32) {
        self.sfx.ticks_per_row = ticks.clamp(limits::TICKS_PER_ROW_MIN, limits::TICKS_PER_ROW_MAX);
    }

    pub fn adjust_ticks_per_row(&mut self, delta: i32) -> bool {
        let before = self.sfx.ticks_per_row;
        self.set_ticks_per_row((before as i32 + delta).max(1) as u32);
        self.sfx.ticks_per_row != before
    }

    pub fn set_frames_per_tick(&mut self, frames: Num<u32, 8>) {
        self.sfx.frames_per_tick = Num::from_raw(frames.to_raw().clamp(
            limits::FRAMES_PER_TICK_MIN << 8,
            limits::FRAMES_PER_TICK_MAX << 8,
        ));
    }

    pub fn adjust_frames_per_tick(&mut self, delta_raw: i32, max_frames: u32) -> bool {
        let before = self.sfx.frames_per_tick;
        let raw = (before.to_raw() as i32 + delta_raw).clamp(
            (limits::FRAMES_PER_TICK_MIN << 8) as i32,
            (max_frames << 8) as i32,
        );
        self.set_frames_per_tick(Num::from_raw(raw as u32));
        self.sfx.frames_per_tick != before
    }
}

impl SfxDocument {
    pub fn row_count(&self) -> usize {
        self.sfx.rows.len()
    }

    fn bump_rows_gen(&mut self) {
        self.rows_gen = self.rows_gen.wrapping_add(1);
    }

    fn update_slot(&mut self, row: usize, f: impl FnOnce(&mut PatternSlot)) -> bool {
        let Some(slot) = self.rows_mut().get_mut(row) else {
            return false;
        };
        let before = *slot;
        f(slot);
        let changed = *slot != before;
        if changed {
            self.bump_rows_gen();
        }
        changed
    }

    pub fn insert_row(&mut self, at: usize) -> bool {
        if self.sfx.rows.len() >= limits::MAX_ROWS {
            return false;
        }
        let end = self.sfx.rows.len();
        self.rows_mut().insert(at.min(end), PatternSlot::EMPTY);
        self.bump_rows_gen();
        true
    }

    pub fn remove_row(&mut self, at: usize) -> bool {
        if self.sfx.rows.len() <= 1 || at >= self.sfx.rows.len() {
            return false;
        }
        self.rows_mut().remove(at);
        self.bump_rows_gen();
        self.sanitize_rows();
        true
    }

    pub fn set_note(&mut self, row: usize, note: u8) {
        let note = if note == NOTE_OFF {
            NOTE_OFF
        } else {
            note.min(self.note_max())
        };
        self.update_slot(row, |slot| slot.note = note);
    }

    pub fn set_row_instrument(&mut self, row: usize, instrument: u8) -> bool {
        if row >= self.sfx.rows.len() {
            return false;
        }
        if instrument != 0 {
            match self.sfx.instruments.get(instrument as usize - 1) {
                Some(inst) if self.instrument_allowed(inst) => {}
                _ => return false,
            }
        }
        self.update_slot(row, |slot| slot.instrument = instrument);
        self.sanitize_rows();
        true
    }

    pub fn set_effect(&mut self, row: usize, effect: PsgEffect) -> bool {
        if row >= self.sfx.rows.len() {
            return false;
        }
        let Some(effect) = self.sanitize_effect(effect, self.sweep_active_at(row)) else {
            return false;
        };
        self.update_slot(row, |slot| slot.effect = effect);
        true
    }
}

impl SfxDocument {
    pub fn add_instrument(&mut self, instrument: Instrument) -> Option<u8> {
        if self.sfx.instruments.len() >= limits::MAX_INSTRUMENTS {
            return None;
        }
        let instrument = self.sanitize_instrument(instrument)?;
        self.instruments_mut().push(instrument);
        Some((self.sfx.instruments.len() - 1) as u8)
    }

    pub fn set_instrument(&mut self, index: u8, instrument: Instrument) -> bool {
        if index as usize >= self.sfx.instruments.len() {
            return false;
        }
        let Some(instrument) = self.sanitize_instrument(instrument) else {
            return false;
        };
        self.instruments_mut()[index as usize] = instrument;
        self.sanitize_rows();
        true
    }

    pub fn remove_instrument(&mut self, index: u8) -> bool {
        if index as usize >= self.sfx.instruments.len() {
            return false;
        }
        self.instruments_mut().remove(index as usize);
        let slot_ref = index + 1;
        let mut changed = false;
        for slot in self.sfx.rows.to_mut() {
            if slot.instrument == slot_ref {
                slot.instrument = 0;
                changed = true;
            } else if slot.instrument > slot_ref {
                slot.instrument -= 1;
                changed = true;
            }
        }
        if changed {
            self.bump_rows_gen();
        }
        self.sanitize_rows();
        true
    }
}

impl SfxDocument {
    pub fn add_wave_table(&mut self, samples: [u8; 16]) -> Option<u8> {
        if self.sfx.wave_tables.len() >= limits::MAX_WAVE_TABLES {
            return None;
        }
        self.wave_tables_mut().push(samples);
        Some((self.sfx.wave_tables.len() - 1) as u8)
    }

    pub fn set_wave_table(&mut self, index: u8, samples: [u8; 16]) -> bool {
        match self.wave_tables_mut().get_mut(index as usize) {
            Some(table) => {
                *table = samples;
                true
            }
            None => false,
        }
    }

    pub fn remove_wave_table(&mut self, index: u8) -> bool {
        if index as usize >= self.sfx.wave_tables.len() {
            return false;
        }
        let referenced = self.sfx.instruments.iter().any(
            |inst| matches!(inst, Instrument::Wave { wave_table, .. } if *wave_table == index),
        );
        if referenced {
            return false;
        }
        self.wave_tables_mut().remove(index as usize);
        for inst in self.sfx.instruments.to_mut() {
            if let Instrument::Wave { wave_table, .. } = inst
                && *wave_table > index
            {
                *wave_table -= 1;
            }
        }
        true
    }
}

impl SfxDocument {
    fn note_max(&self) -> u8 {
        limits::note_max(self.sfx.channel)
    }

    fn instrument_allowed(&self, instrument: &Instrument) -> bool {
        match (self.sfx.channel, instrument) {
            (SfxChannel::SquareSweep, Instrument::Square { .. }) => true,
            (SfxChannel::Square, Instrument::Square { sweep, .. }) => sweep.is_none(),
            (SfxChannel::Wave, Instrument::Wave { .. }) => true,
            (SfxChannel::Noise, Instrument::Noise { .. }) => true,
            _ => false,
        }
    }

    fn sanitize_instrument(&self, instrument: Instrument) -> Option<Instrument> {
        Some(match instrument {
            Instrument::Square {
                duty,
                envelope,
                sweep,
                length,
            } => Instrument::Square {
                duty: duty.min(limits::DUTY_MAX),
                envelope: sanitize_envelope(envelope),
                sweep: sweep.map(|s| SweepSpec {
                    time: s.time.min(limits::SWEEP_TIME_MAX),
                    decreasing: s.decreasing,
                    shift: s.shift.min(limits::SWEEP_SHIFT_MAX),
                }),
                length: length
                    .map(|l| l.clamp(limits::SQUARE_LENGTH_MIN, limits::SQUARE_LENGTH_MAX)),
            },
            Instrument::Wave { wave_table, volume } => {
                if self.sfx.wave_tables.is_empty() {
                    return None;
                }
                Instrument::Wave {
                    wave_table: wave_table.min((self.sfx.wave_tables.len() - 1) as u8),
                    volume: volume.min(limits::volume_max(SfxChannel::Wave)),
                }
            }
            Instrument::Noise {
                envelope,
                short_lfsr,
            } => Instrument::Noise {
                envelope: sanitize_envelope(envelope),
                short_lfsr,
            },
        })
    }

    fn update_sweep_state(&self, slot: &PatternSlot, sweep_active: &mut bool) {
        if slot.instrument != 0
            && let Some(inst) = self.sfx.instruments.get(slot.instrument as usize - 1)
            && self.instrument_allowed(inst)
        {
            *sweep_active = matches!(inst, Instrument::Square { sweep: Some(_), .. });
        }
    }

    pub fn sweep_active_at(&self, row: usize) -> bool {
        let mut sweep_active = false;
        for slot in self.sfx.rows.iter().take(row + 1) {
            self.update_sweep_state(slot, &mut sweep_active);
        }
        sweep_active
    }

    pub fn effect_allowed(&self, effect: PsgEffect, sweep_active: bool) -> bool {
        self.sanitize_effect(effect, sweep_active).is_some()
    }

    fn sanitize_effect(&self, effect: PsgEffect, sweep_active: bool) -> Option<PsgEffect> {
        let channel = self.sfx.channel;
        Some(match effect {
            PsgEffect::None => effect,
            PsgEffect::Arpeggio(x, y) => PsgEffect::Arpeggio(
                x.min(limits::NIBBLE_PARAM_MAX),
                y.min(limits::NIBBLE_PARAM_MAX),
            ),
            PsgEffect::PortamentoUp(_)
            | PsgEffect::PortamentoDown(_)
            | PsgEffect::TonePortamento(_) => {
                if !limits::pitch_slides_allowed(channel, sweep_active) {
                    return None;
                }
                effect
            }
            PsgEffect::Vibrato { speed, depth } => {
                if !limits::period_effects_allowed(channel) {
                    return None;
                }
                PsgEffect::Vibrato {
                    speed: speed.min(limits::NIBBLE_PARAM_MAX),
                    depth: depth.min(limits::NIBBLE_PARAM_MAX),
                }
            }
            PsgEffect::VolumeSlide(v) => {
                PsgEffect::VolumeSlide(v.clamp(-limits::VOLUME_SLIDE_MAX, limits::VOLUME_SLIDE_MAX))
            }
            PsgEffect::NoteCut(t) => PsgEffect::NoteCut(t.min(limits::NOTE_TICK_MAX)),
            PsgEffect::NoteDelay(t) => PsgEffect::NoteDelay(t.min(limits::NOTE_TICK_MAX)),
            PsgEffect::PositionJump(_) | PsgEffect::PatternBreak(_) => return None,
            PsgEffect::SetTicksPerRow(t) => PsgEffect::SetTicksPerRow(
                (t as u32).clamp(limits::TICKS_PER_ROW_MIN, limits::TICKS_PER_ROW_MAX) as u8,
            ),
            PsgEffect::SetFramesPerTick(f) => PsgEffect::SetFramesPerTick(Num::from_raw(
                f.to_raw().max((limits::FRAMES_PER_TICK_MIN << 4) as u16),
            )),
            PsgEffect::SetDuty(d) => {
                if !limits::duty_allowed(channel) {
                    return None;
                }
                PsgEffect::SetDuty(d.min(limits::DUTY_MAX))
            }
            PsgEffect::SetPan { .. } => effect,
            PsgEffect::SetVolume(v) => PsgEffect::SetVolume(v.min(limits::volume_max(channel))),
        })
    }

    fn sanitize_rows(&mut self) {
        let note_max = self.note_max();
        let mut sweep_active = false;
        let mut changed = false;
        for i in 0..self.sfx.rows.len() {
            let mut slot = self.sfx.rows[i];
            if slot.note != NOTE_OFF {
                slot.note = slot.note.min(note_max);
            }
            if slot.instrument != 0
                && !matches!(self.sfx.instruments.get(slot.instrument as usize - 1),
                    Some(inst) if self.instrument_allowed(inst))
            {
                slot.instrument = 0;
            }
            self.update_sweep_state(&slot, &mut sweep_active);
            slot.effect = self
                .sanitize_effect(slot.effect, sweep_active)
                .unwrap_or(PsgEffect::None);
            if self.sfx.rows[i] != slot {
                self.rows_mut()[i] = slot;
                changed = true;
            }
        }
        if changed {
            self.bump_rows_gen();
        }
    }
}

fn sanitize_envelope(envelope: EnvelopeSpec) -> EnvelopeSpec {
    EnvelopeSpec {
        initial_volume: envelope.initial_volume.min(limits::ENV_VOLUME_MAX),
        increasing: envelope.increasing,
        step_time: envelope.step_time.min(limits::ENV_STEP_TIME_MAX),
    }
}

fn note_slot(note: u8, instrument: u8) -> PatternSlot {
    PatternSlot {
        note,
        instrument,
        effect: PsgEffect::None,
    }
}

fn fx_slot(effect: PsgEffect) -> PatternSlot {
    PatternSlot {
        note: NOTE_NONE,
        instrument: 0,
        effect,
    }
}

fn push_fx_run(rows: &mut Vec<PatternSlot>, effect: PsgEffect, count: u8) {
    for _ in 0..count {
        rows.push(fx_slot(effect));
    }
}

fn rand_range(rng: &mut RandomNumberGenerator, lo: i32, hi: i32) -> i32 {
    debug_assert!(lo <= hi, "rand_range span underflow");
    let span = (hi - lo + 1) as u32;
    let r = (rng.next_i32() as u32) >> 16;
    lo + ((r * span) >> 16) as i32
}

fn rand_u8(rng: &mut RandomNumberGenerator, lo: u8, hi: u8) -> u8 {
    rand_range(rng, lo as i32, hi as i32) as u8
}

fn rand_bool(rng: &mut RandomNumberGenerator) -> bool {
    rng.next_i32() & 1 != 0
}

fn rand_envelope(
    rng: &mut RandomNumberGenerator,
    volume_lo: u8,
    volume_hi: u8,
    step_lo: u8,
    step_hi: u8,
) -> EnvelopeSpec {
    EnvelopeSpec {
        initial_volume: rand_u8(rng, volume_lo, volume_hi),
        increasing: false,
        step_time: rand_u8(rng, step_lo, step_hi),
    }
}

fn square_instrument(
    rng: &mut RandomNumberGenerator,
    envelope: EnvelopeSpec,
    sweep: Option<SweepSpec>,
    length: Option<u8>,
) -> Instrument {
    Instrument::Square {
        duty: rand_u8(rng, 0, 3),
        envelope,
        sweep,
        length,
    }
}

fn rand_wave_table(rng: &mut RandomNumberGenerator) -> [u8; 16] {
    let mut samples = [0u8; 32];
    match rand_u8(rng, 0, 2) {
        0 => {
            for (i, s) in samples.iter_mut().enumerate() {
                *s = (i >> 1) as u8;
            }
        }
        1 => {
            for (i, s) in samples.iter_mut().enumerate() {
                *s = if i < 16 { i as u8 } else { (31 - i) as u8 };
            }
        }
        _ => {
            for s in &mut samples {
                *s = rand_u8(rng, 0, 15);
            }
        }
    }
    let mut packed = [0u8; 16];
    for (i, p) in packed.iter_mut().enumerate() {
        *p = (samples[i << 1] << 4) | samples[(i << 1) + 1];
    }
    packed
}
