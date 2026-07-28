use crate::registers::{SOUND3CNT_H, SOUND3CNT_L, SOUND3CNT_X, WAVE_RAM};

const BANK_SELECT: u16 = 1 << 6;
const DAC_ENABLE: u16 = 1 << 7;

/// Uploads a 32-sample wave table using the two banks as a double buffer: CPU
/// writes always target the bank NOT selected for playback, so we write the
/// idle bank and then flip selection — no audible glitch mid-note.
pub fn upload_wave(data: &[u8; 16]) {
    let bank = SOUND3CNT_L.get() & BANK_SELECT;
    for i in 0..8 {
        let half = data[i * 2] as u16 | ((data[i * 2 + 1] as u16) << 8);
        unsafe { ((WAVE_RAM + i * 2) as *mut u16).write_volatile(half) }
    }
    // Play the bank we just wrote (dimension bit stays 0: single 32-sample bank).
    SOUND3CNT_L.set((bank ^ BANK_SELECT) | DAC_ENABLE);
}

/// `volume` is the author-facing 0-4 scale (0 mute, 1 = 25% … 4 = 100%).
fn volume_bits(volume: u8) -> u16 {
    match volume {
        0 => 0,
        1 => 3 << 13, // 25%
        2 => 2 << 13, // 50%
        3 => 1 << 15, // force 75%
        _ => 1 << 13, // 100%
    }
}

pub fn trigger(volume: u8, period: u16) {
    SOUND3CNT_H.set(volume_bits(volume));
    SOUND3CNT_X.set(0x8000 | (period & 0x7FF));
}

/// Wave volume changes freely without a retrigger (unlike square/noise).
pub fn set_volume(volume: u8) {
    SOUND3CNT_H.set(volume_bits(volume));
}

/// Changes pitch without retriggering.
pub fn set_period(period: u16) {
    SOUND3CNT_X.set(period & 0x7FF);
}

/// Silences the channel by disabling its DAC.
pub fn silence() {
    SOUND3CNT_L.set_bits(DAC_ENABLE, 0);
}
