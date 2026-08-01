use crate::registers::{SOUND3CNT_H, SOUND3CNT_L, SOUND3CNT_X, wave_ram_write};

const BANK_SELECT: u16 = 1 << 6;
const DAC_ENABLE: u16 = 1 << 7;

pub fn upload_wave(data: &[u8; 16]) {
    let bank = SOUND3CNT_L.get() & BANK_SELECT;
    for i in 0..8 {
        let half = data[i * 2] as u16 | ((data[i * 2 + 1] as u16) << 8);
        wave_ram_write(i, half);
    }
    SOUND3CNT_L.set((bank ^ BANK_SELECT) | DAC_ENABLE);
}

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
    SOUND3CNT_L.set_bits(DAC_ENABLE, DAC_ENABLE);
    SOUND3CNT_H.set(volume_bits(volume));
    SOUND3CNT_X.set(0x8000 | (period & 0x7FF));
}

pub fn set_volume(volume: u8) {
    SOUND3CNT_H.set(volume_bits(volume));
}

pub fn set_period(period: u16) {
    SOUND3CNT_X.set(period & 0x7FF);
}

pub fn silence() {
    SOUND3CNT_L.set_bits(DAC_ENABLE, 0);
}
