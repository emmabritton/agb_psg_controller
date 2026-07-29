use crate::registers::{Reg16, SOUND1CNT_H, SOUND1CNT_L, SOUND1CNT_X, SOUND2CNT_H, SOUND2CNT_L};

/// Which square channel to drive. `One` has the sweep unit.
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

/// Starts a note. `envelope` is the NRx2-layout byte, `length` is the author's
/// 1-64 duration or `None` to play until note off, `period` is 0-2047.
pub fn trigger(channel: SquareChannel, duty: u8, envelope: u16, length: Option<u8>, period: u16) {
    let (cnt, freq) = channel.regs();
    let length_value = length.map_or(0, |l| (64 - l as u16) & 0x3F);
    cnt.set((envelope << 8) | ((duty as u16) << 6) | length_value);
    let length_enable = (length.is_some() as u16) << 14;
    freq.set(0x8000 | length_enable | (period & 0x7FF));
}

/// Changes pitch without retriggering — glitch-free, used for slides and vibrato.
/// `length_enabled` must match the trigger: this write also covers the
/// length-enable bit, which would otherwise be cleared mid-note.
pub fn set_period(channel: SquareChannel, period: u16, length_enabled: bool) {
    let (_, freq) = channel.regs();
    freq.set(((length_enabled as u16) << 14) | (period & 0x7FF));
}

/// Channel 1 only: NR10-layout sweep byte (bits 4-6 time, bit 3 direction, bits 0-2 shift).
pub fn set_sweep(sweep: u16) {
    SOUND1CNT_L.set(sweep);
}

/// Silences the channel by retriggering with a zeroed envelope (turns the DAC off).
pub fn silence(channel: SquareChannel) {
    let (cnt, freq) = channel.regs();
    cnt.set(0);
    freq.set(0x8000);
}
