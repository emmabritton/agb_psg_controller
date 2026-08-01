use crate::registers::{Reg16, SOUND1CNT_H, SOUND1CNT_L, SOUND1CNT_X, SOUND2CNT_H, SOUND2CNT_L};

#[derive(Clone, Copy)]
pub enum SquareChannel {
    One,
    Two,
}

impl SquareChannel {
    fn regs(self) -> (&'static Reg16, &'static Reg16) {
        match self {
            SquareChannel::One => (&SOUND1CNT_H, &SOUND1CNT_X),
            SquareChannel::Two => (&SOUND2CNT_L, &SOUND2CNT_H),
        }
    }
}

pub fn trigger(channel: SquareChannel, duty: u8, envelope: u16, length: Option<u8>, period: u16) {
    let (cnt, freq) = channel.regs();
    let length_value = length.map_or(0, |l| (64 - l as u16) & 0x3F);
    cnt.set((envelope << 8) | ((duty as u16) << 6) | length_value);
    let length_enable = (length.is_some() as u16) << 14;
    freq.set(0x8000 | length_enable | (period & 0x7FF));
}

pub fn set_period(channel: SquareChannel, period: u16, length_enabled: bool) {
    let (_, freq) = channel.regs();
    freq.set(((length_enabled as u16) << 14) | (period & 0x7FF));
}

pub fn set_sweep(sweep: u16) {
    SOUND1CNT_L.set(sweep);
}

pub fn silence(channel: SquareChannel) {
    let (cnt, freq) = channel.regs();
    cnt.set(0);
    freq.set(0x8000);
}
