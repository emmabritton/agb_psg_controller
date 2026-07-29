#![cfg(feature = "parse")]

use eb_agb_psg_format::*;
use eb_agb_psg_interop::parse::{lower_song, parse_sfx, parse_song};
use eb_agb_psg_interop::*;
use std::collections::BTreeMap;

fn example_song_file() -> SongFile {
    SongFile {
        version: 1,
        frames_per_tick: 2.5,
        ticks_per_row: 6,
        order: vec![0, 1, 0],
        loop_to: Some(1),
        instruments: BTreeMap::from([
            (
                "lead".to_string(),
                PsgInstrument::Square {
                    duty: PsgDuty::D50,
                    envelope: (12, PsgDirection::Down, 3),
                    sweep: None,
                    length: None,
                },
            ),
            (
                "slide".to_string(),
                PsgInstrument::Square {
                    duty: PsgDuty::D25,
                    envelope: (15, PsgDirection::Down, 0),
                    sweep: Some((3, PsgDirection::Down, 2)),
                    length: Some(32),
                },
            ),
            (
                "organ".to_string(),
                PsgInstrument::Wave {
                    table: "bass".to_string(),
                    volume: PsgWaveVolume::V100,
                },
            ),
            (
                "hat".to_string(),
                PsgInstrument::Noise {
                    envelope: (10, PsgDirection::Down, 1),
                    lfsr: PsgLfsr::Long,
                },
            ),
        ]),
        waves: BTreeMap::from([(
            "bass".to_string(),
            "0123456789ABCDEFFEDCBA9876543210".to_string(),
        )]),
        patterns: vec![
            vec![
                "C-4 slide --- | C-4 lead --- | C-3 organ --- | C-5 hat ---".to_string(),
                "--- ..    --- | --- ..   A47 | --- ..    --- | --- ..  ---".to_string(),
                "skip 2".to_string(),
                "off ..    --- | D#4 lead V23 | off ..    --- | --- ..  S02".to_string(),
            ],
            vec!["--- .. --- | C-4 lead T10 | --- .. M03 | C-3 hat ---".to_string()],
        ],
    }
}

#[test]
fn song_roundtrip_through_text() {
    let file = example_song_file();
    let text = file.to_ron().expect("serializes");
    let parsed = parse_song(&text).expect("parses");
    let lowered = lower_song(&file).expect("lowers");
    assert_eq!(parsed, lowered);
}

#[test]
fn song_lowering_shape() {
    let track = lower_song(&example_song_file()).expect("lowers");

    assert_eq!(track.instruments.len(), 4);
    assert_eq!(track.wave_tables.len(), 1);
    assert_eq!(track.patterns.len(), 2);
    assert_eq!(track.pattern_order.as_ref(), &[0, 1, 0]);
    assert_eq!(track.loop_order_index, Some(1));
    assert_eq!(track.ticks_per_row, 6);
    // 2.5 frames per tick in 8.8 fixed point
    assert_eq!(track.frames_per_tick.to_raw(), 0x280);

    // pattern 0: 5 rows (2 written + skip 2 + 1 written), pattern 1: 1 row
    assert_eq!(
        track.patterns[0],
        Pattern {
            start_row: 0,
            num_rows: 5
        }
    );
    assert_eq!(
        track.patterns[1],
        Pattern {
            start_row: 5,
            num_rows: 1
        }
    );
    assert_eq!(track.pattern_data.len(), 6 * NUM_CHANNELS);

    // instruments are indexed in name order: hat=1, lead=2, organ=3, slide=4
    let row0 = &track.pattern_data[0..4];
    assert_eq!(row0[0].note, 25); // C-4
    assert_eq!(row0[0].instrument, 4); // slide
    assert_eq!(row0[1].instrument, 2); // lead
    assert_eq!(row0[2].note, 13); // C-3
    assert_eq!(row0[2].instrument, 3); // organ
    assert_eq!(row0[3].note, 37); // C-5
    assert_eq!(row0[3].instrument, 1); // hat

    // skip rows are empty
    assert_eq!(track.pattern_data[2 * 4], PatternSlot::EMPTY);
    assert_eq!(track.pattern_data[3 * 4 + 3], PatternSlot::EMPTY);

    // row 4: note off + effects
    let row4 = &track.pattern_data[4 * 4..5 * 4];
    assert_eq!(row4[0].note, NOTE_OFF);
    assert_eq!(row4[1].note, 28); // D#4
    assert_eq!(row4[1].effect, PsgEffect::Vibrato { speed: 2, depth: 3 });
    assert_eq!(row4[3].effect, PsgEffect::VolumeSlide(2));

    // arpeggio params split into nibbles (row 1, channel 2)
    assert_eq!(track.pattern_data[5].effect, PsgEffect::Arpeggio(4, 7));

    // instrument definitions
    assert_eq!(
        track.instruments[0], // hat
        Instrument::Noise {
            envelope: EnvelopeSpec {
                initial_volume: 10,
                increasing: false,
                step_time: 1
            },
            short_lfsr: false,
        }
    );
    assert_eq!(
        track.instruments[3], // slide
        Instrument::Square {
            duty: 1,
            envelope: EnvelopeSpec {
                initial_volume: 15,
                increasing: false,
                step_time: 0
            },
            sweep: Some(SweepSpec {
                time: 3,
                decreasing: true,
                shift: 2
            }),
            length: Some(32),
        }
    );

    // wave table bytes decoded from hex
    assert_eq!(track.wave_tables[0][0], 0x01);
    assert_eq!(track.wave_tables[0][15], 0x10);
}

#[test]
fn sfx_roundtrip() {
    let file = SfxFile {
        version: 1,
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
        rows: vec![
            "C-4 boom ---".to_string(),
            "C-3 ..   A30".to_string(),
            "off ..   ---".to_string(),
        ],
    };
    let text = file.to_ron().expect("serializes");
    let sfx = parse_sfx(&text).expect("parses");

    assert_eq!(sfx.channel, SfxChannel::Noise);
    assert_eq!(sfx.rows.len(), 3);
    assert_eq!(sfx.rows[0].note, 25);
    assert_eq!(sfx.rows[1].effect, PsgEffect::Arpeggio(3, 0));
    assert_eq!(sfx.rows[2].note, NOTE_OFF);
}

#[test]
fn rejects_pitch_slide_on_noise() {
    let text = song_text(|f| {
        f.patterns[0][0] = "--- .. --- | --- .. --- | --- .. --- | --- .. D08".to_string()
    });
    let err = parse_song(&text).unwrap_err();
    assert!(err.message.contains("noise"), "{}", err.message);
}

fn song_text(patch: impl Fn(&mut SongFile)) -> String {
    let mut file = example_song_file();
    patch(&mut file);
    file.to_ron().expect("serializes")
}

#[test]
fn rejects_bad_version() {
    let text = song_text(|f| f.version = 2);
    let err = parse_song(&text).unwrap_err();
    assert!(err.message.contains("version"), "{}", err.message);
}

#[test]
fn rejects_wrong_cell_count() {
    let text = song_text(|f| f.patterns[0][0] = "C-4 lead --- | --- .. ---".to_string());
    let err = parse_song(&text).unwrap_err();
    assert!(
        err.message.contains("expected 4 cell(s)"),
        "{}",
        err.message
    );
}

#[test]
fn rejects_unknown_instrument() {
    let text = song_text(|f| {
        f.patterns[0][0] = "C-4 nope --- | --- .. --- | --- .. --- | --- .. ---".to_string()
    });
    let err = parse_song(&text).unwrap_err();
    assert!(
        err.message.contains("unknown instrument"),
        "{}",
        err.message
    );
}

#[test]
fn rejects_instrument_on_wrong_channel() {
    // wave instrument in the noise column
    let text = song_text(|f| {
        f.patterns[0][0] = "--- .. --- | --- .. --- | --- .. --- | C-3 organ ---".to_string()
    });
    let err = parse_song(&text).unwrap_err();
    assert!(err.message.contains("wave instrument"), "{}", err.message);
}

#[test]
fn rejects_sweep_instrument_off_channel_1() {
    let text = song_text(|f| {
        f.patterns[0][0] = "--- .. --- | C-4 slide --- | --- .. --- | --- .. ---".to_string()
    });
    let err = parse_song(&text).unwrap_err();
    assert!(err.message.contains("sweep"), "{}", err.message);
}

#[test]
fn rejects_pitch_slide_on_sweep_instrument() {
    let text = song_text(|f| {
        f.patterns[0][1] = "--- ..    U04 | --- .. --- | --- .. --- | --- .. ---".to_string()
    });
    let err = parse_song(&text).unwrap_err();
    assert!(err.message.contains("sweep"), "{}", err.message);
}

#[test]
fn rejects_wave_note_too_high() {
    let text = song_text(|f| {
        f.patterns[0][0] = "--- .. --- | --- .. --- | C-9 organ --- | --- .. ---".to_string()
    });
    let err = parse_song(&text).unwrap_err();
    assert!(err.message.contains("too high"), "{}", err.message);
}

#[test]
fn rejects_noise_note_too_high() {
    let text = song_text(|f| {
        f.patterns[0][0] = "--- .. --- | --- .. --- | --- .. --- | C-8 hat ---".to_string()
    });
    let err = parse_song(&text).unwrap_err();
    assert!(err.message.contains("too high"), "{}", err.message);
}

#[test]
fn rejects_bad_wave_hex() {
    let text = song_text(|f| {
        f.waves.insert("bass".to_string(), "0123".to_string());
    });
    let err = parse_song(&text).unwrap_err();
    assert!(err.message.contains("32 hex digits"), "{}", err.message);
}

#[test]
fn rejects_unknown_effect() {
    let text = song_text(|f| {
        f.patterns[0][0] = "--- .. Z00 | --- .. --- | --- .. --- | --- .. ---".to_string()
    });
    let err = parse_song(&text).unwrap_err();
    assert!(err.message.contains("unknown effect"), "{}", err.message);
}

/// Effect tokens are length-checked in bytes, so a two-character token can be
/// three bytes long; it must be rejected rather than sliced mid-character.
#[test]
fn rejects_non_ascii_effect() {
    let text = song_text(|f| {
        f.patterns[0][0] = "--- .. é0 | --- .. --- | --- .. --- | --- .. ---".to_string()
    });
    let err = parse_song(&text).unwrap_err();
    assert!(err.message.contains("bad effect"), "{}", err.message);
}

#[test]
fn rejects_zero_skip() {
    let text = song_text(|f| f.patterns[0][2] = "skip 0".to_string());
    let err = parse_song(&text).unwrap_err();
    assert!(err.message.contains("skip count"), "{}", err.message);
}

#[test]
fn rejects_order_out_of_range() {
    let text = song_text(|f| f.order = vec![0, 7]);
    let err = parse_song(&text).unwrap_err();
    assert!(err.message.contains("order entry"), "{}", err.message);
}

#[test]
fn rejects_position_jump_out_of_range() {
    let text = song_text(|f| {
        f.patterns[0][0] = "--- .. B09 | --- .. --- | --- .. --- | --- .. ---".to_string()
    });
    let err = parse_song(&text).unwrap_err();
    assert!(err.message.contains("position jump"), "{}", err.message);
}

#[test]
fn rejects_wave_volume_effect_out_of_range() {
    let text = song_text(|f| {
        f.patterns[0][0] = "--- .. --- | --- .. --- | --- .. M09 | --- .. ---".to_string()
    });
    let err = parse_song(&text).unwrap_err();
    assert!(
        err.message.contains("volume must be 0-4"),
        "{}",
        err.message
    );
}

#[test]
fn rejects_loop_out_of_range() {
    let text = song_text(|f| f.loop_to = Some(9));
    let err = parse_song(&text).unwrap_err();
    assert!(err.message.contains("loop_to"), "{}", err.message);
}

#[test]
fn parses_handwritten_ron() {
    let text = r##"(
        version: 1,
        frames_per_tick: 2.0,
        ticks_per_row: 4,
        order: [0],
        instruments: {
            "beep": Square(envelope: (12, Down, 3)),
        },
        patterns: [
            [
                "--- .. --- | C-4 beep --- | --- .. --- | --- .. ---",
                "skip 3",
            ],
        ],
    )"##;
    let track = parse_song(text).expect("parses");
    assert_eq!(track.patterns[0].num_rows, 4);
    assert_eq!(track.pattern_data[1].note, 25);
    assert_eq!(track.loop_order_index, None);
    match track.instruments[0] {
        Instrument::Square { duty, .. } => assert_eq!(duty, 2), // default D50
        _ => panic!("expected square"),
    }
}
