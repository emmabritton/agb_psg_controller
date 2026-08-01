use crate::parse::ParseError;
use crate::track::*;
use eb_agb_psg_format::{
    FORMAT_VERSION, PsgChannel, PsgDirection, PsgDuty, PsgInstrument, PsgLfsr, PsgWaveVolume,
    SfxFile,
};
use std::collections::BTreeMap;

fn err<T>(message: impl Into<String>) -> Result<T, ParseError> {
    Err(ParseError {
        message: message.into(),
    })
}

pub fn sfx_to_file(sfx: &Sfx) -> Result<SfxFile, ParseError> {
    let waves: BTreeMap<String, String> = sfx
        .wave_tables
        .iter()
        .enumerate()
        .map(|(i, table)| {
            let hex: String = table.iter().map(|b| format!("{b:02x}")).collect();
            (wave_name(i as u8), hex)
        })
        .collect();

    let mut instruments = BTreeMap::new();
    for (i, inst) in sfx.instruments.iter().enumerate() {
        instruments.insert(instrument_name(i as u8), convert_instrument(inst, sfx)?);
    }

    let mut rows = Vec::new();
    let mut empty_run = 0usize;
    for slot in sfx.rows.iter() {
        if *slot == PatternSlot::EMPTY {
            empty_run += 1;
            continue;
        }
        flush_skip(&mut rows, &mut empty_run);
        rows.push(cell_str(slot, sfx)?);
    }
    flush_skip(&mut rows, &mut empty_run);

    Ok(SfxFile {
        version: FORMAT_VERSION,
        frames_per_tick: sfx.frames_per_tick.to_raw() as f32 / 256.0,
        ticks_per_row: sfx.ticks_per_row,
        channel: match sfx.channel {
            SfxChannel::SquareSweep => PsgChannel::SquareSweep,
            SfxChannel::Square => PsgChannel::Square,
            SfxChannel::Wave => PsgChannel::Wave,
            SfxChannel::Noise => PsgChannel::Noise,
        },
        instruments,
        waves,
        rows,
    })
}

/// 0-based vector index to the 1-based display name the row cells also use.
fn instrument_name(index: u8) -> String {
    format!("i{:03}", index as u16 + 1)
}

fn wave_name(index: u8) -> String {
    format!("w{:03}", index as u16 + 1)
}

fn flush_skip(rows: &mut Vec<String>, empty_run: &mut usize) {
    match *empty_run {
        0 => {}
        // A lone empty row reads better as a cell than as `skip 1`.
        1 => rows.push("--- .. ---".to_string()),
        n => rows.push(format!("skip {n}")),
    }
    *empty_run = 0;
}

fn convert_instrument(inst: &Instrument, sfx: &Sfx) -> Result<PsgInstrument, ParseError> {
    Ok(match inst {
        Instrument::Square {
            duty,
            envelope,
            sweep,
            length,
        } => PsgInstrument::Square {
            duty: match duty {
                0 => PsgDuty::D12_5,
                1 => PsgDuty::D25,
                2 => PsgDuty::D50,
                3 => PsgDuty::D75,
                d => return err(format!("invalid duty {d}")),
            },
            envelope: convert_envelope(envelope),
            sweep: sweep.map(|s| (s.time, direction(!s.decreasing), s.shift)),
            length: *length,
        },
        Instrument::Wave { wave_table, volume } => {
            if *wave_table as usize >= sfx.wave_tables.len() {
                return err(format!(
                    "wave instrument refers to missing table {wave_table}"
                ));
            }
            PsgInstrument::Wave {
                table: wave_name(*wave_table),
                volume: match volume {
                    0 => PsgWaveVolume::V0,
                    1 => PsgWaveVolume::V25,
                    2 => PsgWaveVolume::V50,
                    3 => PsgWaveVolume::V75,
                    4 => PsgWaveVolume::V100,
                    v => return err(format!("invalid wave volume {v}")),
                },
            }
        }
        Instrument::Noise {
            envelope,
            short_lfsr,
        } => PsgInstrument::Noise {
            envelope: convert_envelope(envelope),
            lfsr: if *short_lfsr {
                PsgLfsr::Short
            } else {
                PsgLfsr::Long
            },
        },
    })
}

fn convert_envelope(env: &EnvelopeSpec) -> (u8, PsgDirection, u8) {
    (env.initial_volume, direction(env.increasing), env.step_time)
}

fn direction(up: bool) -> PsgDirection {
    if up {
        PsgDirection::Up
    } else {
        PsgDirection::Down
    }
}

fn cell_str(slot: &PatternSlot, sfx: &Sfx) -> Result<String, ParseError> {
    let instrument = match slot.instrument {
        0 => "..".to_string(),
        i if i as usize <= sfx.instruments.len() => instrument_name(i - 1),
        i => return err(format!("row refers to missing instrument {i}")),
    };
    Ok(format!(
        "{} {} {}",
        note_str(slot.note)?,
        instrument,
        effect_str(&slot.effect)?
    ))
}

fn note_str(note: u8) -> Result<String, ParseError> {
    const NAMES: [&str; 12] = [
        "C-", "C#", "D-", "D#", "E-", "F-", "F#", "G-", "G#", "A-", "A#", "B-",
    ];
    match note {
        NOTE_NONE => Ok("---".to_string()),
        NOTE_OFF => Ok("off".to_string()),
        1..=NOTE_MAX => {
            let idx = (note - 1) as usize;
            Ok(format!("{}{}", NAMES[idx % 12], idx / 12 + 2))
        }
        n => err(format!("invalid note value {n}")),
    }
}

fn effect_str(effect: &PsgEffect) -> Result<String, ParseError> {
    let (letter, param) = match effect {
        PsgEffect::None => return Ok("---".to_string()),
        PsgEffect::Arpeggio(x, y) => ('A', (x & 0xF) << 4 | (y & 0xF)),
        PsgEffect::PortamentoUp(p) => ('U', *p),
        PsgEffect::PortamentoDown(p) => ('D', *p),
        PsgEffect::TonePortamento(p) => ('T', *p),
        PsgEffect::Vibrato { speed, depth } => ('V', (speed & 0xF) << 4 | (depth & 0xF)),
        PsgEffect::VolumeSlide(v) => ('S', *v as u8),
        PsgEffect::NoteCut(t) => ('C', *t),
        PsgEffect::NoteDelay(t) => ('Q', *t),
        PsgEffect::PositionJump(t) => ('B', *t),
        PsgEffect::PatternBreak(r) => ('K', *r),
        PsgEffect::SetTicksPerRow(t) => ('R', *t),
        PsgEffect::SetFramesPerTick(f) => {
            let raw = f.to_raw();
            if raw > 0xFF {
                return err(format!(
                    "frames-per-tick effect raw value {raw} exceeds Fxx"
                ));
            }
            ('F', raw as u8)
        }
        PsgEffect::SetDuty(d) => ('W', *d),
        PsgEffect::SetPan { left, right } => ('P', (*left as u8) << 1 | *right as u8),
        PsgEffect::SetVolume(v) => ('M', *v),
    };
    Ok(format!("{letter}{param:02X}"))
}
