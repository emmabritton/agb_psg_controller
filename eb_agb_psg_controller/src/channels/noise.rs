use crate::registers::{SOUND4CNT_H, SOUND4CNT_L};

const SHORT_LFSR: u16 = 1 << 3;

/// Starts a noise burst. `envelope` is the NRx2-layout byte; `poly` is the
/// `(shift << 4) | divisor` byte from `NOISE_TABLE`.
pub fn trigger(envelope: u16, poly: u8, short_lfsr: bool) {
    SOUND4CNT_L.set(envelope << 8);
    let poly = poly as u16;
    let width = if short_lfsr { SHORT_LFSR } else { 0 };
    // Poly bits 4-7 (shift) land in register bits 4-7, divisor in bits 0-2.
    SOUND4CNT_H.set(0x8000 | (poly & 0xF0) | width | (poly & 0x07));
}

/// Changes pitch without retriggering.
pub fn set_poly(poly: u8, short_lfsr: bool) {
    let poly = poly as u16;
    let width = if short_lfsr { SHORT_LFSR } else { 0 };
    SOUND4CNT_H.set((poly & 0xF0) | width | (poly & 0x07));
}

/// Silences the channel by retriggering with a zeroed envelope.
pub fn silence() {
    SOUND4CNT_L.set(0);
    SOUND4CNT_H.set(0x8000);
}
