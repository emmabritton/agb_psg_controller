pub struct Reg16(usize);

impl Reg16 {
    pub const fn new(address: usize) -> Self {
        Self(address)
    }

    #[cfg(not(feature = "host"))]
    pub fn set(&self, value: u16) {
        unsafe { (self.0 as *mut u16).write_volatile(value) }
    }

    #[cfg(not(feature = "host"))]
    pub fn get(&self) -> u16 {
        unsafe { (self.0 as *const u16).read_volatile() }
    }

    #[cfg(feature = "host")]
    pub fn set(&self, value: u16) {
        host::reg_write(self.0, value)
    }

    #[cfg(feature = "host")]
    pub fn get(&self) -> u16 {
        host::reg_read(self.0)
    }

    pub fn set_bits(&self, mask: u16, value: u16) {
        let current = self.get();
        let wanted = (current & !mask) | (value & mask);
        if wanted != current {
            self.set(wanted);
        }
    }
}

pub const SOUND1CNT_L: Reg16 = Reg16::new(0x0400_0060);
pub const SOUND1CNT_H: Reg16 = Reg16::new(0x0400_0062);
pub const SOUND1CNT_X: Reg16 = Reg16::new(0x0400_0064);
pub const SOUND2CNT_L: Reg16 = Reg16::new(0x0400_0068);
pub const SOUND2CNT_H: Reg16 = Reg16::new(0x0400_006C);
pub const SOUND3CNT_L: Reg16 = Reg16::new(0x0400_0070);
pub const SOUND3CNT_H: Reg16 = Reg16::new(0x0400_0072);
pub const SOUND3CNT_X: Reg16 = Reg16::new(0x0400_0074);
pub const SOUND4CNT_L: Reg16 = Reg16::new(0x0400_0078);
pub const SOUND4CNT_H: Reg16 = Reg16::new(0x0400_007C);
pub const SOUNDCNT_L: Reg16 = Reg16::new(0x0400_0080);
pub const SOUNDCNT_H: Reg16 = Reg16::new(0x0400_0082);
pub const SOUNDCNT_X: Reg16 = Reg16::new(0x0400_0084);
#[cfg_attr(feature = "host", allow(dead_code))]
pub const WAVE_RAM: usize = 0x0400_0090;

pub const PSG_RATIO_MASK: u16 = 0b11;
pub const MASTER_ENABLE: u16 = 1 << 7;

pub fn wave_ram_write(i: usize, half: u16) {
    #[cfg(not(feature = "host"))]
    unsafe {
        ((WAVE_RAM + i * 2) as *mut u16).write_volatile(half)
    }
    #[cfg(feature = "host")]
    host::wave_write(i, half);
}

#[cfg(feature = "host")]
pub mod host {
    extern crate std;
    use alloc::vec::Vec;
    use core::cell::RefCell;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum WriteEvent {
        Reg { addr: usize, value: u16 },
        WaveRam { index: usize, value: u16 },
    }

    const BASE: usize = 0x0400_0060;
    const REGS: usize = 19;

    std::thread_local! {
        static SHADOW: RefCell<[u16; REGS]> = const { RefCell::new([0; REGS]) };
        static LOG: RefCell<Vec<WriteEvent>> = const { RefCell::new(Vec::new()) };
    }

    pub(super) fn reg_write(addr: usize, value: u16) {
        SHADOW.with(|s| s.borrow_mut()[(addr - BASE) / 2] = value);
        LOG.with(|l| l.borrow_mut().push(WriteEvent::Reg { addr, value }));
    }

    pub(super) fn reg_read(addr: usize) -> u16 {
        SHADOW.with(|s| s.borrow()[(addr - BASE) / 2])
    }

    pub(super) fn wave_write(index: usize, value: u16) {
        LOG.with(|l| l.borrow_mut().push(WriteEvent::WaveRam { index, value }));
    }

    pub fn reg(addr: usize) -> u16 {
        reg_read(addr)
    }

    pub fn take_writes() -> Vec<WriteEvent> {
        LOG.with(|l| core::mem::take(&mut *l.borrow_mut()))
    }

    pub fn reset() {
        SHADOW.with(|s| *s.borrow_mut() = [0; REGS]);
        LOG.with(|l| l.borrow_mut().clear());
    }
}
