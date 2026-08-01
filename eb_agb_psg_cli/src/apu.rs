use eb_agb_psg_controller::host::WriteEvent;

pub const SAMPLE_RATE: u32 = 44_100;

const SOUND1CNT_L: usize = 0x0400_0060;
const SOUND1CNT_H: usize = 0x0400_0062;
const SOUND1CNT_X: usize = 0x0400_0064;
const SOUND2CNT_L: usize = 0x0400_0068;
const SOUND2CNT_H: usize = 0x0400_006C;
const SOUND3CNT_L: usize = 0x0400_0070;
const SOUND3CNT_H: usize = 0x0400_0072;
const SOUND3CNT_X: usize = 0x0400_0074;
const SOUND4CNT_L: usize = 0x0400_0078;
const SOUND4CNT_H: usize = 0x0400_007C;
const SOUNDCNT_L: usize = 0x0400_0080;
const SOUNDCNT_H: usize = 0x0400_0082;
const SOUNDCNT_X: usize = 0x0400_0084;

const DUTY: [u8; 4] = [0b0000_0001, 0b1000_0001, 0b1000_0111, 0b0111_1110];

#[derive(Default)]
struct Envelope {
    volume: u8,
    increasing: bool,
    step_time: u8,
    timer: u8,
    dac_on: bool,
    initial_volume: u8,
}

impl Envelope {
    fn set(&mut self, nrx2: u8) {
        self.initial_volume = nrx2 >> 4;
        self.increasing = nrx2 & 0x08 != 0;
        self.step_time = nrx2 & 0x07;
        self.dac_on = nrx2 & 0xF8 != 0;
    }

    fn trigger(&mut self) {
        self.volume = self.initial_volume;
        self.timer = self.step_time;
    }

    fn clock(&mut self) {
        if self.step_time == 0 {
            return;
        }
        self.timer -= 1;
        if self.timer > 0 {
            return;
        }
        self.timer = self.step_time;
        if self.increasing {
            self.volume = (self.volume + 1).min(15);
        } else {
            self.volume = self.volume.saturating_sub(1);
        }
    }
}

#[derive(Default)]
struct Square {
    on: bool,
    duty: u8,
    period: u16,
    phase: f64,
    env: Envelope,
    length: u16,
    length_enabled: bool,
    sweep_time: u8,
    sweep_decreasing: bool,
    sweep_shift: u8,
    sweep_timer: u8,
    sweep_shadow: u16,
}

impl Square {
    fn trigger(&mut self) {
        self.on = self.env.dac_on;
        self.phase = 0.0;
        self.env.trigger();
        if self.length == 0 {
            self.length = 64;
        }
        self.sweep_shadow = self.period;
        self.sweep_timer = if self.sweep_time == 0 {
            8
        } else {
            self.sweep_time
        };
    }

    fn clock_length(&mut self) {
        if self.length_enabled && self.length > 0 {
            self.length -= 1;
            if self.length == 0 {
                self.on = false;
            }
        }
    }

    fn clock_sweep(&mut self) {
        if self.sweep_timer > 0 {
            self.sweep_timer -= 1;
        }
        if self.sweep_timer > 0 {
            return;
        }
        self.sweep_timer = if self.sweep_time == 0 {
            8
        } else {
            self.sweep_time
        };
        if self.sweep_time == 0 {
            return;
        }
        let delta = self.sweep_shadow >> self.sweep_shift;
        let next = if self.sweep_decreasing {
            self.sweep_shadow.saturating_sub(delta)
        } else {
            self.sweep_shadow + delta
        };
        if next > 2047 {
            self.on = false;
        } else if self.sweep_shift > 0 {
            self.sweep_shadow = next;
            self.period = next;
        }
    }

    fn sample(&mut self) -> f32 {
        if !self.on || !self.env.dac_on {
            return 0.0;
        }
        let freq = 131_072.0 / (2048 - self.period.min(2047)) as f64;
        self.phase = (self.phase + 8.0 * freq / SAMPLE_RATE as f64) % 8.0;
        let high = DUTY[self.duty as usize] >> (self.phase as u32) & 1 != 0;
        let out = if high { self.env.volume } else { 0 };
        out as f32 / 7.5 - 1.0
    }
}

#[derive(Default)]
struct Wave {
    on: bool,
    period: u16,
    pos: f64,
}

#[derive(Default)]
struct Noise {
    on: bool,
    lfsr: u16,
    short: bool,
    divisor: u8,
    shift: u8,
    acc: f64,
    env: Envelope,
    length: u16,
    length_enabled: bool,
}

impl Noise {
    fn clock_length(&mut self) {
        if self.length_enabled && self.length > 0 {
            self.length -= 1;
            if self.length == 0 {
                self.on = false;
            }
        }
    }

    fn sample(&mut self) -> f32 {
        if !self.on || !self.env.dac_on {
            return 0.0;
        }
        let r = if self.divisor == 0 {
            0.5
        } else {
            self.divisor as f64
        };
        let freq = 524_288.0 / r / 2f64.powi(self.shift as i32 + 1);
        self.acc += freq / SAMPLE_RATE as f64;
        while self.acc >= 1.0 {
            self.acc -= 1.0;
            let bit = (self.lfsr ^ (self.lfsr >> 1)) & 1;
            self.lfsr >>= 1;
            self.lfsr |= bit << 14;
            if self.short {
                self.lfsr = (self.lfsr & !(1 << 6)) | (bit << 6);
            }
        }
        let out = if self.lfsr & 1 == 0 {
            self.env.volume
        } else {
            0
        };
        out as f32 / 7.5 - 1.0
    }
}

#[derive(Default)]
pub struct Apu {
    sq: [Square; 2],
    wave: Wave,
    noise: Noise,
    wave_ram: [[u8; 16]; 2],
    sound3cnt_l: u16,
    sound3cnt_h: u16,
    soundcnt_l: u16,
    soundcnt_h: u16,
    soundcnt_x: u16,
    seq_acc: f64,
    seq_step: u8,
}

impl Apu {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn apply(&mut self, events: &[WriteEvent]) {
        for event in events {
            match *event {
                WriteEvent::Reg { addr, value } => self.write(addr, value),
                WriteEvent::WaveRam { index, value } => {
                    let bank = ((self.sound3cnt_l >> 6) & 1) as usize ^ 1;
                    self.wave_ram[bank][index * 2] = value as u8;
                    self.wave_ram[bank][index * 2 + 1] = (value >> 8) as u8;
                }
            }
        }
    }

    fn write(&mut self, addr: usize, value: u16) {
        match addr {
            SOUND1CNT_L => {
                let sq = &mut self.sq[0];
                sq.sweep_shift = (value & 0x07) as u8;
                sq.sweep_decreasing = value & 0x08 != 0;
                sq.sweep_time = ((value >> 4) & 0x07) as u8;
            }
            SOUND1CNT_H => self.write_square_duty_env(0, value),
            SOUND2CNT_L => self.write_square_duty_env(1, value),
            SOUND1CNT_X => self.write_square_control(0, value),
            SOUND2CNT_H => self.write_square_control(1, value),
            SOUND3CNT_L => {
                self.sound3cnt_l = value;
                if value & 0x80 == 0 {
                    self.wave.on = false;
                }
            }
            SOUND3CNT_H => self.sound3cnt_h = value,
            SOUND3CNT_X => {
                self.wave.period = value & 0x7FF;
                if value & 0x8000 != 0 {
                    self.wave.on = self.sound3cnt_l & 0x80 != 0;
                    self.wave.pos = 0.0;
                }
            }
            SOUND4CNT_L => {
                let noise = &mut self.noise;
                noise.length = 64 - (value & 0x3F);
                noise.env.set((value >> 8) as u8);
                if !noise.env.dac_on {
                    noise.on = false;
                }
            }
            SOUND4CNT_H => {
                let noise = &mut self.noise;
                noise.divisor = (value & 0x07) as u8;
                noise.short = value & 0x08 != 0;
                noise.shift = ((value >> 4) & 0x0F) as u8;
                noise.length_enabled = value & 0x4000 != 0;
                if value & 0x8000 != 0 {
                    noise.lfsr = 0x7FFF;
                    noise.on = noise.env.dac_on;
                    noise.env.trigger();
                    if noise.length == 0 {
                        noise.length = 64;
                    }
                }
            }
            SOUNDCNT_L => self.soundcnt_l = value,
            SOUNDCNT_H => self.soundcnt_h = value,
            SOUNDCNT_X => self.soundcnt_x = value,
            _ => {}
        }
    }

    fn write_square_duty_env(&mut self, ch: usize, value: u16) {
        let sq = &mut self.sq[ch];
        sq.length = 64 - (value & 0x3F);
        sq.duty = ((value >> 6) & 0x03) as u8;
        sq.env.set((value >> 8) as u8);
        if !sq.env.dac_on {
            sq.on = false;
        }
    }

    fn write_square_control(&mut self, ch: usize, value: u16) {
        let sq = &mut self.sq[ch];
        sq.period = value & 0x7FF;
        sq.length_enabled = value & 0x4000 != 0;
        if value & 0x8000 != 0 {
            sq.trigger();
        }
    }

    fn clock_sequencer(&mut self) {
        let step = self.seq_step;
        self.seq_step = (step + 1) % 8;
        if step.is_multiple_of(2) {
            self.sq[0].clock_length();
            self.sq[1].clock_length();
            self.noise.clock_length();
        }
        if step == 2 || step == 6 {
            self.sq[0].clock_sweep();
        }
        if step == 7 {
            self.sq[0].env.clock();
            self.sq[1].env.clock();
            self.noise.env.clock();
        }
    }

    fn wave_sample(&mut self) -> f32 {
        if !self.wave.on {
            return 0.0;
        }
        let volume = if self.sound3cnt_h & 0x8000 != 0 {
            0.75
        } else {
            match (self.sound3cnt_h >> 13) & 0x03 {
                0 => 0.0,
                1 => 1.0,
                2 => 0.5,
                _ => 0.25,
            }
        };
        let freq = 2_097_152.0 / (2048 - self.wave.period.min(2047)) as f64;
        self.wave.pos = (self.wave.pos + freq / SAMPLE_RATE as f64) % 32.0;
        let pos = self.wave.pos as usize;
        let bank = ((self.sound3cnt_l >> 6) & 1) as usize;
        let byte = self.wave_ram[bank][pos / 2];
        let nibble = if pos.is_multiple_of(2) {
            byte >> 4
        } else {
            byte & 0x0F
        };
        (nibble as f32 / 7.5 - 1.0) * volume
    }

    pub fn render(&mut self, samples: &mut Vec<f32>, count: usize) {
        for _ in 0..count {
            self.seq_acc += 512.0 / SAMPLE_RATE as f64;
            while self.seq_acc >= 1.0 {
                self.seq_acc -= 1.0;
                self.clock_sequencer();
            }

            let mut mix = 0.0f32;
            if self.soundcnt_x & 0x80 != 0 {
                let cnt_l = self.soundcnt_l;
                let enabled = |ch: usize| cnt_l & (0x0100 << ch | 0x1000 << ch) != 0;
                if enabled(0) {
                    mix += self.sq[0].sample();
                }
                if enabled(1) {
                    mix += self.sq[1].sample();
                }
                if enabled(2) {
                    mix += self.wave_sample();
                }
                if enabled(3) {
                    mix += self.noise.sample();
                }
                let master =
                    ((self.soundcnt_l & 0x07).max((self.soundcnt_l >> 4) & 0x07)) as f32 / 7.0;
                let ratio = match self.soundcnt_h & 0x03 {
                    0 => 0.25,
                    1 => 0.5,
                    _ => 1.0,
                };
                mix *= master * ratio * 0.25;
            }
            samples.push(mix);
        }
    }
}
