//! Parses `.pmus` / `.psfx` text into the ROM model, with validation.

use crate::track::*;
use agb_fixnum::Num;
use alloc::borrow::Cow;
use eb_agb_psg_format::{
    FORMAT_VERSION, PsgChannel, PsgDirection, PsgDuty, PsgInstrument, PsgLfsr, PsgWaveVolume,
    SfxFile, SongFile,
};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ParseError {}

fn err<T>(message: impl Into<String>) -> Result<T, ParseError> {
    Err(ParseError {
        message: message.into(),
    })
}

pub fn parse_song(text: &str) -> Result<Track, ParseError> {
    let file = SongFile::from_ron(text).map_err(|e| ParseError {
        message: e.to_string(),
    })?;
    lower_song(&file)
}

pub fn parse_sfx(text: &str) -> Result<Sfx, ParseError> {
    let file = SfxFile::from_ron(text).map_err(|e| ParseError {
        message: e.to_string(),
    })?;
    lower_sfx(&file)
}

/// The parts every document has: version, timing, waves and instruments.
struct Header {
    frames_per_tick: Num<u32, 8>,
    wave_tables: Vec<[u8; 16]>,
    instruments: Vec<Instrument>,
    instrument_index: BTreeMap<String, InstrumentRef>,
}

fn lower_header(
    version: u32,
    frames_per_tick: f32,
    ticks_per_row: u32,
    waves: &BTreeMap<String, String>,
    instruments: &BTreeMap<String, PsgInstrument>,
) -> Result<Header, ParseError> {
    if version != FORMAT_VERSION {
        return err(format!(
            "unsupported format version {version} (expected {FORMAT_VERSION})"
        ));
    }
    let frames_per_tick = convert_frames_per_tick(frames_per_tick)?;
    check_ticks_per_row(ticks_per_row)?;

    let (wave_tables, wave_index) = build_waves(waves)?;
    let (instruments, instrument_index) = build_instruments(instruments, &wave_index)?;

    Ok(Header {
        frames_per_tick,
        wave_tables,
        instruments,
        instrument_index,
    })
}

pub fn lower_song(file: &SongFile) -> Result<Track, ParseError> {
    let Header {
        frames_per_tick,
        wave_tables,
        instruments,
        instrument_index,
    } = lower_header(
        file.version,
        file.frames_per_tick,
        file.ticks_per_row,
        &file.waves,
        &file.instruments,
    )?;

    if file.patterns.is_empty() {
        return err("song has no patterns");
    }
    if file.patterns.len() > 256 {
        return err("too many patterns (max 256)");
    }
    if file.order.is_empty() {
        return err("`order` must not be empty");
    }
    let mut pattern_order = Vec::with_capacity(file.order.len());
    for (i, &entry) in file.order.iter().enumerate() {
        if entry as usize >= file.patterns.len() {
            return err(format!(
                "order entry {i} refers to pattern {entry}, but there are only {} patterns",
                file.patterns.len()
            ));
        }
        pattern_order.push(entry as u8);
    }
    if let Some(loop_to) = file.loop_to
        && loop_to as usize >= file.order.len()
    {
        return err(format!(
            "loop_to {loop_to} is outside the order list (length {})",
            file.order.len()
        ));
    }

    const SONG_COLUMNS: [ColumnKind; NUM_CHANNELS] = [
        ColumnKind::SquareSweep,
        ColumnKind::Square,
        ColumnKind::Wave,
        ColumnKind::Noise,
    ];

    let mut pattern_data = Vec::new();
    let mut patterns = Vec::with_capacity(file.patterns.len());
    for (pattern_idx, lines) in file.patterns.iter().enumerate() {
        let ctx = RowContext {
            pattern: Some(pattern_idx),
            order_len: Some(file.order.len()),
        };
        let rows = lower_rows(lines, &SONG_COLUMNS, &instrument_index, &ctx)?;
        let num_rows = rows.len() / NUM_CHANNELS;
        patterns.push(Pattern {
            start_row: (pattern_data.len() / NUM_CHANNELS) as u32,
            num_rows: num_rows as u32,
        });
        pattern_data.extend(rows);
    }

    Ok(Track {
        instruments: Cow::Owned(instruments),
        wave_tables: Cow::Owned(wave_tables),
        pattern_data: Cow::Owned(pattern_data),
        patterns: Cow::Owned(patterns),
        pattern_order: Cow::Owned(pattern_order),
        frames_per_tick,
        ticks_per_row: file.ticks_per_row,
        loop_order_index: file.loop_to,
    })
}

pub fn lower_sfx(file: &SfxFile) -> Result<Sfx, ParseError> {
    let Header {
        frames_per_tick,
        wave_tables,
        instruments,
        instrument_index,
    } = lower_header(
        file.version,
        file.frames_per_tick,
        file.ticks_per_row,
        &file.waves,
        &file.instruments,
    )?;

    if file.rows.is_empty() {
        return err("sfx has no rows");
    }
    let column = match file.channel {
        PsgChannel::SquareSweep => ColumnKind::SquareSweep,
        PsgChannel::Square => ColumnKind::Square,
        PsgChannel::Wave => ColumnKind::Wave,
        PsgChannel::Noise => ColumnKind::Noise,
    };
    let ctx = RowContext {
        pattern: None,
        order_len: None,
    };
    let rows = lower_rows(&file.rows, &[column], &instrument_index, &ctx)?;

    Ok(Sfx {
        channel: match file.channel {
            PsgChannel::SquareSweep => SfxChannel::SquareSweep,
            PsgChannel::Square => SfxChannel::Square,
            PsgChannel::Wave => SfxChannel::Wave,
            PsgChannel::Noise => SfxChannel::Noise,
        },
        instruments: Cow::Owned(instruments),
        wave_tables: Cow::Owned(wave_tables),
        rows: Cow::Owned(rows),
        frames_per_tick,
        ticks_per_row: file.ticks_per_row,
    })
}

fn convert_frames_per_tick(value: f32) -> Result<Num<u32, 8>, ParseError> {
    if !value.is_finite() || value <= 0.0 || value > 255.0 {
        return err(format!(
            "frames_per_tick must be greater than 0 and at most 255, got {value}"
        ));
    }
    Ok(Num::from_raw((value * 256.0).round() as u32))
}

fn check_ticks_per_row(value: u32) -> Result<(), ParseError> {
    if !(1..=31).contains(&value) {
        return err(format!("ticks_per_row must be 1-31, got {value}"));
    }
    Ok(())
}

type WaveTable = (Vec<[u8; 16]>, BTreeMap<String, u8>);

fn build_waves(waves: &BTreeMap<String, String>) -> Result<WaveTable, ParseError> {
    if waves.len() > 255 {
        return err("too many wave tables (max 255)");
    }
    let mut tables = Vec::with_capacity(waves.len());
    let mut index = BTreeMap::new();
    for (i, (name, hex)) in waves.iter().enumerate() {
        if hex.len() != 32 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return err(format!(
                "wave \"{name}\": expected exactly 32 hex digits, got \"{hex}\""
            ));
        }
        let mut table = [0u8; 16];
        for (j, byte) in table.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&hex[j * 2..j * 2 + 2], 16).unwrap();
        }
        tables.push(table);
        index.insert(name.clone(), i as u8);
    }
    Ok((tables, index))
}

/// What an instrument name resolves to: its 1-based index and enough kind
/// information to validate cell placement.
#[derive(Clone, Copy)]
struct InstrumentRef {
    index: u8,
    kind: ColumnKind,
    has_sweep: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ColumnKind {
    SquareSweep,
    Square,
    Wave,
    Noise,
}

impl ColumnKind {
    fn name(self) -> &'static str {
        match self {
            ColumnKind::SquareSweep => "square+sweep",
            ColumnKind::Square => "square",
            ColumnKind::Wave => "wave",
            ColumnKind::Noise => "noise",
        }
    }

    fn accepts(self, instrument: ColumnKind) -> bool {
        match self {
            // Both square columns take square instruments; sweep is checked separately.
            ColumnKind::SquareSweep | ColumnKind::Square => {
                matches!(instrument, ColumnKind::SquareSweep | ColumnKind::Square)
            }
            other => other == instrument,
        }
    }
}

type InstrumentTable = (Vec<Instrument>, BTreeMap<String, InstrumentRef>);

fn build_instruments(
    source: &BTreeMap<String, PsgInstrument>,
    wave_index: &BTreeMap<String, u8>,
) -> Result<InstrumentTable, ParseError> {
    if source.len() > 255 {
        return err("too many instruments (max 255)");
    }
    let mut instruments = Vec::with_capacity(source.len());
    let mut index = BTreeMap::new();
    for (i, (name, inst)) in source.iter().enumerate() {
        let (lowered, kind, has_sweep) = match inst {
            PsgInstrument::Square {
                duty,
                envelope,
                sweep,
                length,
            } => {
                let envelope = convert_envelope(name, envelope)?;
                let sweep = (*sweep)
                    .map(|(time, dir, shift)| {
                        if time > 7 || shift > 7 {
                            return err(format!(
                                "instrument \"{name}\": sweep time and shift must be 0-7"
                            ));
                        }
                        Ok(SweepSpec {
                            time,
                            decreasing: dir == PsgDirection::Down,
                            shift,
                        })
                    })
                    .transpose()?;
                if let Some(length) = length
                    && !(1..=64).contains(length)
                {
                    return err(format!("instrument \"{name}\": length must be 1-64"));
                }
                let duty = match duty {
                    PsgDuty::D12_5 => 0,
                    PsgDuty::D25 => 1,
                    PsgDuty::D50 => 2,
                    PsgDuty::D75 => 3,
                };
                (
                    Instrument::Square {
                        duty,
                        envelope,
                        sweep,
                        length: *length,
                    },
                    if sweep.is_some() {
                        ColumnKind::SquareSweep
                    } else {
                        ColumnKind::Square
                    },
                    sweep.is_some(),
                )
            }
            PsgInstrument::Wave { table, volume } => {
                let Some(&wave_table) = wave_index.get(table) else {
                    return err(format!(
                        "instrument \"{name}\" refers to unknown wave \"{table}\""
                    ));
                };
                let volume = match volume {
                    PsgWaveVolume::V0 => 0,
                    PsgWaveVolume::V25 => 1,
                    PsgWaveVolume::V50 => 2,
                    PsgWaveVolume::V75 => 3,
                    PsgWaveVolume::V100 => 4,
                };
                (
                    Instrument::Wave { wave_table, volume },
                    ColumnKind::Wave,
                    false,
                )
            }
            PsgInstrument::Noise { envelope, lfsr } => (
                Instrument::Noise {
                    envelope: convert_envelope(name, envelope)?,
                    short_lfsr: *lfsr == PsgLfsr::Short,
                },
                ColumnKind::Noise,
                false,
            ),
        };
        instruments.push(lowered);
        index.insert(
            name.clone(),
            InstrumentRef {
                index: (i + 1) as u8,
                kind,
                has_sweep,
            },
        );
    }
    Ok((instruments, index))
}

fn convert_envelope(
    name: &str,
    &(volume, dir, step): &(u8, PsgDirection, u8),
) -> Result<EnvelopeSpec, ParseError> {
    if volume > 15 {
        return err(format!(
            "instrument \"{name}\": envelope volume must be 0-15"
        ));
    }
    if step > 7 {
        return err(format!(
            "instrument \"{name}\": envelope step time must be 0-7"
        ));
    }
    Ok(EnvelopeSpec {
        initial_volume: volume,
        increasing: dir == PsgDirection::Up,
        step_time: step,
    })
}

struct RowContext {
    pattern: Option<usize>,
    /// `Some` for songs (enables and bounds-checks `Bxx`); `None` for SFX,
    /// where jump effects are rejected.
    order_len: Option<usize>,
}

impl RowContext {
    fn describe(&self, row: usize, channel: usize, num_channels: usize) -> String {
        let mut s = String::new();
        if let Some(p) = self.pattern {
            s.push_str(&format!("pattern {p}, "));
        }
        s.push_str(&format!("row {row}"));
        if num_channels > 1 {
            s.push_str(&format!(", channel {}", channel + 1));
        }
        s
    }
}

fn lower_rows(
    lines: &[String],
    columns: &[ColumnKind],
    instrument_index: &BTreeMap<String, InstrumentRef>,
    ctx: &RowContext,
) -> Result<Vec<PatternSlot>, ParseError> {
    let num_channels = columns.len();
    let mut slots = Vec::new();
    // Tracks whether the most recent instrument per column has hardware sweep,
    // to reject pitch-slide effects that would fight it.
    let mut sweep_active = vec![false; num_channels];
    let mut row = 0usize;

    for line in lines {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("skip ") {
            let count: usize = rest.trim().parse().map_err(|_| ParseError {
                message: format!("bad skip count \"{rest}\" after row {row}"),
            })?;
            if count == 0 {
                return err(format!("skip count must be at least 1 (after row {row})"));
            }
            slots.extend(core::iter::repeat_n(
                PatternSlot::EMPTY,
                count * num_channels,
            ));
            row += count;
            continue;
        }

        let cells: Vec<&str> = line.split('|').collect();
        if cells.len() != num_channels {
            return err(format!(
                "{}: expected {num_channels} cell(s), got {}",
                ctx.describe(row, 0, 1),
                cells.len()
            ));
        }
        for (channel, cell) in cells.iter().enumerate() {
            let slot = parse_cell(
                cell,
                columns[channel],
                instrument_index,
                &mut sweep_active[channel],
                ctx,
            )
            .map_err(|msg| ParseError {
                message: format!("{}: {msg}", ctx.describe(row, channel, num_channels)),
            })?;
            slots.push(slot);
        }
        row += 1;
    }

    if row == 0 {
        return err(format!(
            "{}: pattern has no rows",
            ctx.pattern
                .map(|p| format!("pattern {p}"))
                .unwrap_or_else(|| "sfx".to_string())
        ));
    }
    if row > 256 {
        return err(format!(
            "{}: too many rows ({row}, max 256)",
            ctx.pattern
                .map(|p| format!("pattern {p}"))
                .unwrap_or_else(|| "sfx".to_string())
        ));
    }
    Ok(slots)
}

fn parse_cell(
    cell: &str,
    column: ColumnKind,
    instrument_index: &BTreeMap<String, InstrumentRef>,
    sweep_active: &mut bool,
    ctx: &RowContext,
) -> Result<PatternSlot, String> {
    let tokens: Vec<&str> = cell.split_whitespace().collect();
    if tokens.len() != 3 {
        return Err(format!(
            "expected `note instrument effect`, got {} token(s) in \"{}\"",
            tokens.len(),
            cell.trim()
        ));
    }

    let note = parse_note(tokens[0])?;
    let is_pitched = (1..=NOTE_MAX).contains(&note);
    match column {
        ColumnKind::Wave if is_pitched && note > NOTE_MAX - WAVE_NOTE_OFFSET => {
            return Err(format!(
                "note {} is too high for the wave channel (max B-8)",
                tokens[0]
            ));
        }
        ColumnKind::Noise if is_pitched && note > NOISE_NOTE_MAX => {
            return Err(format!(
                "note {} is too high for the noise channel (max B-6)",
                tokens[0]
            ));
        }
        _ => {}
    }

    let instrument = if tokens[1] == ".." {
        0
    } else {
        let Some(inst) = instrument_index.get(tokens[1]) else {
            return Err(format!("unknown instrument \"{}\"", tokens[1]));
        };
        if !column.accepts(inst.kind) {
            return Err(format!(
                "instrument \"{}\" is a {} instrument, but this is the {} channel",
                tokens[1],
                inst.kind.name(),
                column.name()
            ));
        }
        if inst.has_sweep && column != ColumnKind::SquareSweep {
            return Err(format!(
                "instrument \"{}\" has a sweep, which only works on channel 1",
                tokens[1]
            ));
        }
        *sweep_active = inst.has_sweep;
        inst.index
    };

    let effect = parse_effect(tokens[2])?;
    validate_effect(&effect, column, *sweep_active, ctx)?;

    Ok(PatternSlot {
        note,
        instrument,
        effect,
    })
}

fn parse_note(s: &str) -> Result<u8, String> {
    if s == "---" {
        return Ok(NOTE_NONE);
    }
    if s.eq_ignore_ascii_case("off") {
        return Ok(NOTE_OFF);
    }
    let b = s.as_bytes();
    if b.len() != 3 {
        return Err(format!("bad note \"{s}\""));
    }
    let base = match b[0].to_ascii_uppercase() {
        b'C' => 0,
        b'D' => 2,
        b'E' => 4,
        b'F' => 5,
        b'G' => 7,
        b'A' => 9,
        b'B' => 11,
        _ => return Err(format!("bad note \"{s}\"")),
    };
    let semitone = match b[1] {
        b'-' => base,
        b'#' if base != 4 && base != 11 => base + 1, // no E#/B#
        _ => return Err(format!("bad note \"{s}\"")),
    };
    let octave = (b[2] as char)
        .to_digit(10)
        .filter(|o| (2..=9).contains(o))
        .ok_or_else(|| format!("bad note \"{s}\" (octaves are 2-9)"))?;
    Ok(((octave - 2) * 12 + semitone + 1) as u8)
}

fn parse_effect(s: &str) -> Result<PsgEffect, String> {
    if s == "---" {
        return Ok(PsgEffect::None);
    }
    // Byte length, so the ASCII check has to come first: a 3-byte token can be
    // two characters, and slicing it below would split one of them.
    if s.len() != 3 || !s.is_ascii() {
        return Err(format!("bad effect \"{s}\""));
    }
    let param = u8::from_str_radix(&s[1..], 16)
        .map_err(|_| format!("bad effect parameter in \"{s}\" (two hex digits)"))?;
    Ok(match s.as_bytes()[0].to_ascii_uppercase() {
        b'A' => PsgEffect::Arpeggio(param >> 4, param & 0xF),
        b'U' => PsgEffect::PortamentoUp(param),
        b'D' => PsgEffect::PortamentoDown(param),
        b'T' => PsgEffect::TonePortamento(param),
        b'V' => PsgEffect::Vibrato {
            speed: param >> 4,
            depth: param & 0xF,
        },
        b'S' => PsgEffect::VolumeSlide(param as i8),
        b'C' => PsgEffect::NoteCut(param),
        b'Q' => PsgEffect::NoteDelay(param),
        b'B' => PsgEffect::PositionJump(param),
        b'K' => PsgEffect::PatternBreak(param),
        b'R' => PsgEffect::SetTicksPerRow(param),
        b'F' => PsgEffect::SetFramesPerTick(Num::from_raw(param as u16)),
        b'W' => PsgEffect::SetDuty(param),
        b'P' => PsgEffect::SetPan {
            left: param & 2 != 0,
            right: param & 1 != 0,
        },
        b'M' => PsgEffect::SetVolume(param),
        _ => return Err(format!("unknown effect \"{s}\"")),
    })
}

fn validate_effect(
    effect: &PsgEffect,
    column: ColumnKind,
    sweep_active: bool,
    ctx: &RowContext,
) -> Result<(), String> {
    let is_square = matches!(column, ColumnKind::SquareSweep | ColumnKind::Square);
    match effect {
        PsgEffect::PortamentoUp(_)
        | PsgEffect::PortamentoDown(_)
        | PsgEffect::TonePortamento(_)
            if sweep_active =>
        {
            Err("pitch slides cannot be combined with a sweep instrument".to_string())
        }
        PsgEffect::PortamentoUp(_)
        | PsgEffect::PortamentoDown(_)
        | PsgEffect::TonePortamento(_)
        | PsgEffect::Vibrato { .. }
            if column == ColumnKind::Noise =>
        {
            Err(
                "period-based pitch effects do not work on the noise channel (arpeggio does)"
                    .to_string(),
            )
        }
        PsgEffect::PositionJump(target) => match ctx.order_len {
            None => Err("position jump is not allowed in an sfx".to_string()),
            Some(len) if *target as usize >= len => Err(format!(
                "position jump target {target} is outside the order list (length {len})"
            )),
            Some(_) => Ok(()),
        },
        PsgEffect::PatternBreak(_) if ctx.order_len.is_none() => {
            Err("pattern break is not allowed in an sfx".to_string())
        }
        PsgEffect::SetTicksPerRow(t) if !(1..=31).contains(t) => {
            Err(format!("ticks per row must be 1-31, got {t}"))
        }
        PsgEffect::SetFramesPerTick(f) if f.to_raw() == 0 => {
            Err("frames per tick must be greater than 0".to_string())
        }
        PsgEffect::SetDuty(_) if !is_square => {
            Err("duty only applies to square channels".to_string())
        }
        PsgEffect::SetDuty(d) if *d > 3 => Err(format!("duty must be 0-3, got {d}")),
        PsgEffect::SetVolume(v) => {
            let max = if column == ColumnKind::Wave { 4 } else { 15 };
            if *v > max {
                Err(format!(
                    "volume must be 0-{max} on the {} channel, got {v}",
                    column.name()
                ))
            } else {
                Ok(())
            }
        }
        _ => Ok(()),
    }
}
