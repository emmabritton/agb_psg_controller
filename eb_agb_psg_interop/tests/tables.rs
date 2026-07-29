use eb_agb_psg_interop::{NOISE_TABLE, NOTE_PERIODS};

fn noise_frequency(poly: u8) -> f64 {
    let shift = (poly >> 4) as i32;
    let divisor = (poly & 0xF) as f64;
    let divisor = if divisor == 0.0 { 0.5 } else { divisor };
    524288.0 / divisor / 2f64.powi(shift + 1)
}

#[test]
fn noise_table_is_strictly_ascending_in_frequency() {
    for pair in NOISE_TABLE.windows(2) {
        assert!(
            noise_frequency(pair[0]) < noise_frequency(pair[1]),
            "{:#04x} -> {:#04x}",
            pair[0],
            pair[1]
        );
    }
    for &poly in &NOISE_TABLE {
        assert!(poly >> 4 <= 13, "shift 14/15 is invalid: {poly:#04x}");
    }
}

#[test]
fn note_periods_match_formula() {
    for (i, &period) in NOTE_PERIODS.iter().enumerate() {
        let freq = 65.40639133_f64 * 2f64.powf(i as f64 / 12.0);
        let expected = 2048 - (131072.0 / freq).round() as i32;
        assert_eq!(period as i32, expected, "semitone index {}", i + 1);
        assert!(period < 2048);
    }
    // A-4 (index 34, table position 33) should be 440 Hz to the nearest period unit
    let a4 = NOTE_PERIODS[33];
    let freq = 131072.0 / (2048 - a4 as i32) as f64;
    assert!((freq - 440.0).abs() < 1.0, "A-4 is {freq} Hz");
}

#[test]
fn note_periods_are_monotonic() {
    for pair in NOTE_PERIODS.windows(2) {
        assert!(pair[0] <= pair[1]);
    }
}
