//! The crate's job is RON in and RON out, unchanged. Semantic validation lives
//! in `agb_psg_interop` and is tested there.

use eb_agb_psg_format::*;
use std::collections::BTreeMap;

fn song() -> SongFile {
    SongFile {
        version: FORMAT_VERSION,
        frames_per_tick: 2.5,
        ticks_per_row: 6,
        order: vec![0, 1, 0],
        loop_to: Some(1),
        instruments: BTreeMap::from([
            (
                "lead".to_string(),
                PsgInstrument::Square {
                    duty: PsgDuty::D25,
                    envelope: (12, PsgDirection::Down, 3),
                    sweep: Some((3, PsgDirection::Up, 2)),
                    length: Some(32),
                },
            ),
            (
                "organ".to_string(),
                PsgInstrument::Wave {
                    table: "tri".to_string(),
                    volume: PsgWaveVolume::V75,
                },
            ),
            (
                "hat".to_string(),
                PsgInstrument::Noise {
                    envelope: (10, PsgDirection::Down, 1),
                    lfsr: PsgLfsr::Short,
                },
            ),
        ]),
        waves: BTreeMap::from([(
            "tri".to_string(),
            "0123456789ABCDEFFEDCBA9876543210".to_string(),
        )]),
        patterns: vec![vec![
            "C-4 lead --- | --- .. --- | C-3 organ --- | C-5 hat ---".to_string(),
            "skip 3".to_string(),
        ]],
    }
}

#[test]
fn song_survives_a_ron_roundtrip() {
    let file = song();
    let parsed = SongFile::from_ron(&file.to_ron().expect("serializes")).expect("parses");
    assert_eq!(parsed, file);
}

#[test]
fn sfx_survives_a_ron_roundtrip() {
    let file = SfxFile {
        version: FORMAT_VERSION,
        frames_per_tick: 1.0,
        ticks_per_row: 2,
        channel: PsgChannel::Noise,
        instruments: BTreeMap::from([(
            "boom".to_string(),
            PsgInstrument::Noise {
                envelope: (15, PsgDirection::Down, 2),
                lfsr: PsgLfsr::Long,
            },
        )]),
        waves: BTreeMap::new(),
        rows: vec!["C-4 boom ---".to_string(), "off ..   ---".to_string()],
    };
    let parsed = SfxFile::from_ron(&file.to_ron().expect("serializes")).expect("parses");
    assert_eq!(parsed, file);
}

/// `waves`, `loop_to` and the instrument fields with defaults may be omitted.
#[test]
fn optional_fields_may_be_omitted() {
    let text = r##"(
        version: 1,
        frames_per_tick: 2.0,
        ticks_per_row: 4,
        order: [0],
        instruments: { "beep": Square(envelope: (12, Down, 3)) },
        patterns: [["--- .. --- | C-4 beep --- | --- .. --- | --- .. ---"]],
    )"##;
    let file = SongFile::from_ron(text).expect("parses");
    assert_eq!(file.loop_to, None);
    assert!(file.waves.is_empty());
    assert_eq!(
        file.instruments["beep"],
        PsgInstrument::Square {
            duty: PsgDuty::D50,
            envelope: (12, PsgDirection::Down, 3),
            sweep: None,
            length: None,
        }
    );
}

#[test]
fn rejects_malformed_ron() {
    assert!(SongFile::from_ron("(version: 1,").is_err());
    // `ticks_per_row` missing entirely
    assert!(SongFile::from_ron("(version: 1, frames_per_tick: 1.0)").is_err());
}
