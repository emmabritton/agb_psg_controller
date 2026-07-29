//! Volatile access to the PSG registers. agb exposes no PSG API and keeps its
//! own register wrapper private, so this crate owns these addresses.

pub struct Reg16(usize);

impl Reg16 {
    pub const fn new(address: usize) -> Self {
        Self(address)
    }

    pub fn set(&self, value: u16) {
        unsafe { (self.0 as *mut u16).write_volatile(value) }
    }

    pub fn get(&self) -> u16 {
        unsafe { (self.0 as *const u16).read_volatile() }
    }

    /// Replaces only the bits covered by `mask`, skipping the write entirely
    /// when it wouldn't change the register — `frame()` re-asserts a couple of
    /// these unconditionally every frame, so most calls are no-ops.
    pub fn set_bits(&self, mask: u16, value: u16) {
        let current = self.get();
        let wanted = (current & !mask) | (value & mask);
        if wanted != current {
            self.set(wanted);
        }
    }
}

/// Channel 1 sweep (NR10).
pub const SOUND1CNT_L: Reg16 = Reg16::new(0x0400_0060);
/// Channel 1 duty/length/envelope (NR11/NR12).
pub const SOUND1CNT_H: Reg16 = Reg16::new(0x0400_0062);
/// Channel 1 period/control (NR13/NR14): bits 0-10 period, bit 14 length enable, bit 15 restart.
pub const SOUND1CNT_X: Reg16 = Reg16::new(0x0400_0064);
/// Channel 2 duty/length/envelope.
pub const SOUND2CNT_L: Reg16 = Reg16::new(0x0400_0068);
/// Channel 2 period/control.
pub const SOUND2CNT_H: Reg16 = Reg16::new(0x0400_006C);
/// Channel 3 control: bit 5 dimension, bit 6 bank select, bit 7 channel enable.
pub const SOUND3CNT_L: Reg16 = Reg16::new(0x0400_0070);
/// Channel 3 length (bits 0-7), volume (bits 13-14), force-75% (bit 15).
pub const SOUND3CNT_H: Reg16 = Reg16::new(0x0400_0072);
/// Channel 3 period/control.
pub const SOUND3CNT_X: Reg16 = Reg16::new(0x0400_0074);
/// Channel 4 length/envelope.
pub const SOUND4CNT_L: Reg16 = Reg16::new(0x0400_0078);
/// Channel 4 divisor (bits 0-2), LFSR width (bit 3), shift (bits 4-7), length enable (bit 14), restart (bit 15).
pub const SOUND4CNT_H: Reg16 = Reg16::new(0x0400_007C);
/// PSG master volume (bits 0-2 right, 4-6 left) and per-channel enables (bits 8-11 right, 12-15 left).
pub const SOUNDCNT_L: Reg16 = Reg16::new(0x0400_0080);
/// Mixing: bits 0-1 are the PSG output ratio (0 = 25%, 1 = 50%, 2 = 100%).
pub const SOUNDCNT_H: Reg16 = Reg16::new(0x0400_0082);
/// Bit 7 is the master sound enable; while it is clear all other sound registers are dead.
pub const SOUNDCNT_X: Reg16 = Reg16::new(0x0400_0084);
/// Wave RAM: 16 bytes accessed as 8 u16s; writes go to the bank NOT selected in SOUND3CNT_L.
pub const WAVE_RAM: usize = 0x0400_0090;

pub const PSG_RATIO_MASK: u16 = 0b11;
pub const MASTER_ENABLE: u16 = 1 << 7;
