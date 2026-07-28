//! Effect state machines: pure math over `ChannelState`, no register access,
//! so behaviour is testable without listening.

use agb::fixnum::Num;
use eb_agb_psg_interop::{NOISE_NOTE_MAX, NOTE_MAX, NOTE_PERIODS, PsgEffect, WAVE_NOTE_OFFSET};

use crate::{ChannelState, dirty, silence_state};

/// round(8 * sin(2*pi*i / 32)) — used for vibrato, scaled by depth.
const SINE8: [i8; 32] = [
    0, 2, 3, 4, 6, 7, 7, 8, 8, 8, 7, 7, 6, 4, 3, 2, 0, -2, -3, -4, -6, -7, -7, -8, -8, -8, -7, -7,
    -6, -4, -3, -2,
];

/// Row-level changes that outlive a single channel: tempo and jumps, applied
/// by the player after the whole row is processed.
#[derive(Default)]
pub struct TimingChange {
    pub ticks_per_row: Option<u32>,
    pub frames_per_tick: Option<Num<u32, 8>>,
    pub jump: Option<Jump>,
}

pub enum Jump {
    /// Continue at this order position, row 0.
    Position(u8),
    /// Continue at this row of the next order entry.
    Break(u8),
}

/// Applies the immediate (tick 0) part of the row's effect.
pub fn apply_row_effect(state: &mut ChannelState, channel: usize, timing: &mut TimingChange) {
    match state.effect {
        PsgEffect::VolumeSlide(delta) => change_volume(state, channel, delta),
        PsgEffect::SetVolume(volume) => {
            let current = state.volume as i8;
            change_volume(state, channel, volume as i8 - current);
        }
        PsgEffect::SetDuty(duty) => {
            state.duty = duty;
            // Duty lives in the same register as the envelope; rewriting it
            // mid-note has unreliable envelope side effects on hardware, so we
            // apply it with a retrigger like volume changes — but only when
            // there is a note to retrigger.
            if state.note != 0 {
                state.dirty |= dirty::RETRIGGER;
            }
        }
        PsgEffect::SetPan { left, right } => {
            state.pan_left = left;
            state.pan_right = right;
            state.dirty |= dirty::PAN;
        }
        PsgEffect::NoteCut(0) => silence_state(state),
        PsgEffect::SetTicksPerRow(ticks) => timing.ticks_per_row = Some(ticks as u32),
        PsgEffect::SetFramesPerTick(frames) => {
            // 4.4 -> 8.8 fixed point
            timing.frames_per_tick = Some(Num::from_raw((frames.to_raw() as u32) << 4));
        }
        PsgEffect::PositionJump(target) => timing.jump = Some(Jump::Position(target)),
        PsgEffect::PatternBreak(row) => timing.jump = Some(Jump::Break(row)),
        _ => {}
    }
}

/// Applies the continuous part of the row's effect on ticks 1..ticks_per_row.
pub fn apply_tick_effect(state: &mut ChannelState, channel: usize, tick: u32) {
    match state.effect {
        PsgEffect::Arpeggio(x, y) => {
            let offset = match tick % 3 {
                1 => x,
                2 => y,
                _ => 0,
            };
            apply_arpeggio(state, channel, offset);
        }
        PsgEffect::PortamentoUp(step) => {
            state.period = (state.period + step as u16).min(2047);
            state.dirty |= dirty::PERIOD;
        }
        PsgEffect::PortamentoDown(step) => {
            state.period = state.period.saturating_sub(step as u16);
            state.dirty |= dirty::PERIOD;
        }
        PsgEffect::TonePortamento(step) => {
            let step = step as u16;
            if state.period < state.target_period {
                state.period = (state.period + step).min(state.target_period);
            } else {
                state.period = state.period.saturating_sub(step).max(state.target_period);
            }
            state.dirty |= dirty::PERIOD;
        }
        PsgEffect::Vibrato { speed, depth } => {
            state.vibrato_phase = state.vibrato_phase.wrapping_add(speed);
            let sine = SINE8[(state.vibrato_phase & 31) as usize] as i16;
            state.vib_offset = (sine * depth as i16) >> 2;
            state.dirty |= dirty::PERIOD;
        }
        PsgEffect::NoteCut(cut_tick) if cut_tick as u32 == tick => silence_state(state),
        _ => {}
    }
}

fn apply_arpeggio(state: &mut ChannelState, channel: usize, offset: u8) {
    if state.note == 0 {
        // Nothing sounding to transpose; without this the offset alone would be
        // read as a semitone index.
        return;
    }
    if channel == 3 {
        state.noise_note = (state.note + offset).clamp(1, NOISE_NOTE_MAX);
        state.dirty |= dirty::PERIOD;
        return;
    }
    let mut note = state.note + offset;
    if channel == 2 {
        note += WAVE_NOTE_OFFSET;
    }
    if (1..=NOTE_MAX).contains(&note) {
        state.period = NOTE_PERIODS[(note - 1) as usize];
        state.dirty |= dirty::PERIOD;
    }
}

fn change_volume(state: &mut ChannelState, channel: usize, delta: i8) {
    let max = if channel == 2 { 4 } else { 15 };
    let volume = (state.volume as i8 + delta).clamp(0, max) as u8;
    if volume == state.volume {
        return;
    }
    state.volume = volume;
    if state.note == 0 {
        // Silent channel: keep the value for the next note-on, which writes it
        // as part of triggering. Touching hardware here would either restart a
        // note the song has already ended or, on noise, trigger with no pitch.
        return;
    }
    if channel == 2 {
        // Wave volume is a free register write.
        state.dirty |= dirty::VOLUME;
    } else {
        // Square/noise volume only takes effect on retrigger (hardware quirk);
        // accept the phase-reset click, as documented.
        state.dirty |= dirty::RETRIGGER;
    }
}
