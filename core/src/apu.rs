// Audio Processing Unit: 2 pulse channels (sweep on ch1), wave, noise.
// Generates interleaved stereo f32 at 48 kHz.

pub const SAMPLE_RATE: u32 = 48000;
const CLOCK: u32 = 4_194_304;

const DUTY: [[u8; 8]; 4] = [
    [0, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 1, 1, 1],
    [0, 1, 1, 1, 1, 1, 1, 0],
];

#[derive(Default)]
struct Envelope {
    initial: u8,
    increase: bool,
    period: u8,
    volume: u8,
    timer: u8,
}

impl Envelope {
    fn write(&mut self, val: u8) {
        self.initial = val >> 4;
        self.increase = val & 0x08 != 0;
        self.period = val & 0x07;
    }
    fn read(&self) -> u8 {
        (self.initial << 4) | (self.increase as u8) << 3 | self.period
    }
    fn trigger(&mut self) {
        self.volume = self.initial;
        self.timer = if self.period == 0 { 8 } else { self.period };
    }
    fn clock(&mut self) {
        if self.period == 0 {
            return;
        }
        self.timer = self.timer.saturating_sub(1);
        if self.timer == 0 {
            self.timer = self.period;
            if self.increase && self.volume < 15 {
                self.volume += 1;
            } else if !self.increase && self.volume > 0 {
                self.volume -= 1;
            }
        }
    }
    fn dac_on(&self) -> bool {
        self.initial != 0 || self.increase
    }
}

#[derive(Default)]
struct Pulse {
    enabled: bool,
    duty: u8,
    duty_pos: usize,
    freq: u16,
    timer: i32,
    length: u16,
    length_enable: bool,
    env: Envelope,
    // sweep (channel 1 only)
    sweep_period: u8,
    sweep_negate: bool,
    sweep_shift: u8,
    sweep_timer: u8,
    sweep_enabled: bool,
    sweep_shadow: u16,
    sweep_negate_used: bool,
}

impl Pulse {
    fn output(&self) -> u8 {
        if !self.enabled {
            return 0;
        }
        DUTY[self.duty as usize][self.duty_pos] * self.env.volume
    }

    fn tick(&mut self, cycles: i32) {
        self.timer -= cycles;
        while self.timer <= 0 {
            self.timer += ((2048 - self.freq as i32) * 4).max(4);
            self.duty_pos = (self.duty_pos + 1) & 7;
        }
    }

    fn clock_length(&mut self) {
        if self.length_enable && self.length > 0 {
            self.length -= 1;
            if self.length == 0 {
                self.enabled = false;
            }
        }
    }

    fn sweep_calc(&mut self) -> u16 {
        let delta = self.sweep_shadow >> self.sweep_shift;
        let new = if self.sweep_negate {
            self.sweep_negate_used = true;
            self.sweep_shadow.wrapping_sub(delta)
        } else {
            self.sweep_shadow.wrapping_add(delta)
        };
        if new > 2047 {
            self.enabled = false;
        }
        new
    }

    fn clock_sweep(&mut self) {
        self.sweep_timer = self.sweep_timer.saturating_sub(1);
        if self.sweep_timer == 0 {
            self.sweep_timer = if self.sweep_period == 0 { 8 } else { self.sweep_period };
            if self.sweep_enabled && self.sweep_period != 0 {
                let new = self.sweep_calc();
                if new <= 2047 && self.sweep_shift != 0 {
                    self.sweep_shadow = new;
                    self.freq = new;
                    self.sweep_calc();
                }
            }
        }
    }

    fn trigger(&mut self, has_sweep: bool) {
        self.enabled = true;
        if self.length == 0 {
            self.length = 64;
        }
        self.timer = ((2048 - self.freq as i32) * 4).max(4);
        self.env.trigger();
        if has_sweep {
            self.sweep_shadow = self.freq;
            self.sweep_timer = if self.sweep_period == 0 { 8 } else { self.sweep_period };
            self.sweep_enabled = self.sweep_period != 0 || self.sweep_shift != 0;
            self.sweep_negate_used = false;
            if self.sweep_shift != 0 {
                self.sweep_calc();
            }
        }
        if !self.env.dac_on() {
            self.enabled = false;
        }
    }
}

#[derive(Default)]
struct Wave {
    enabled: bool,
    dac: bool,
    freq: u16,
    timer: i32,
    length: u16,
    length_enable: bool,
    volume_shift: u8, // 0=mute,1=100%,2=50%,3=25%
    pos: usize,
    sample: u8,
}

impl Wave {
    fn tick(&mut self, cycles: i32, ram: &[u8; 16]) {
        self.timer -= cycles;
        while self.timer <= 0 {
            self.timer += ((2048 - self.freq as i32) * 2).max(2);
            self.pos = (self.pos + 1) & 31;
            let byte = ram[self.pos / 2];
            self.sample = if self.pos & 1 == 0 { byte >> 4 } else { byte & 0x0F };
        }
    }
    fn output(&self) -> u8 {
        if !self.enabled || !self.dac || self.volume_shift == 0 {
            return 0;
        }
        self.sample >> (self.volume_shift - 1)
    }
    fn clock_length(&mut self) {
        if self.length_enable && self.length > 0 {
            self.length -= 1;
            if self.length == 0 {
                self.enabled = false;
            }
        }
    }
}

#[derive(Default)]
struct Noise {
    enabled: bool,
    length: u16,
    length_enable: bool,
    env: Envelope,
    shift: u8,
    width7: bool,
    divisor: u8,
    timer: i32,
    lfsr: u16,
}

impl Noise {
    fn period(&self) -> i32 {
        let d = if self.divisor == 0 { 8 } else { self.divisor as i32 * 16 };
        d << self.shift
    }
    fn tick(&mut self, cycles: i32) {
        self.timer -= cycles;
        while self.timer <= 0 {
            self.timer += self.period().max(8);
            let xor = (self.lfsr ^ (self.lfsr >> 1)) & 1;
            self.lfsr = (self.lfsr >> 1) | (xor << 14);
            if self.width7 {
                self.lfsr = (self.lfsr & !(1 << 6)) | (xor << 6);
            }
        }
    }
    fn output(&self) -> u8 {
        if !self.enabled || self.lfsr & 1 != 0 {
            return 0;
        }
        self.env.volume
    }
    fn clock_length(&mut self) {
        if self.length_enable && self.length > 0 {
            self.length -= 1;
            if self.length == 0 {
                self.enabled = false;
            }
        }
    }
}

pub struct Apu {
    enabled: bool,
    ch1: Pulse,
    ch2: Pulse,
    ch3: Wave,
    ch4: Noise,
    pub wave_ram: [u8; 16],
    nr50: u8,
    nr51: u8,

    frame_seq: u8,
    frame_timer: i32,
    sample_timer: f64,
    cycles_per_sample: f64,

    // Simple RC high-pass state per output channel.
    cap_l: f32,
    cap_r: f32,

    pub samples: Vec<f32>, // interleaved stereo
}

impl Apu {
    pub fn new() -> Self {
        Apu {
            enabled: true,
            ch1: Pulse::default(),
            ch2: Pulse::default(),
            ch3: Wave::default(),
            ch4: Noise { lfsr: 0x7FFF, ..Default::default() },
            wave_ram: [0; 16],
            nr50: 0x77,
            nr51: 0xF3,
            frame_seq: 0,
            frame_timer: 8192,
            sample_timer: 0.0,
            cycles_per_sample: CLOCK as f64 / SAMPLE_RATE as f64,
            cap_l: 0.0,
            cap_r: 0.0,
            samples: Vec::with_capacity(4096),
        }
    }

    pub fn tick(&mut self, cycles: u32) {
        let cycles = cycles as i32;
        if self.enabled {
            self.ch1.tick(cycles);
            self.ch2.tick(cycles);
            self.ch3.tick(cycles, &self.wave_ram);
            self.ch4.tick(cycles);

            self.frame_timer -= cycles;
            while self.frame_timer <= 0 {
                self.frame_timer += 8192;
                self.clock_frame_seq();
            }
        }

        self.sample_timer += cycles as f64;
        while self.sample_timer >= self.cycles_per_sample {
            self.sample_timer -= self.cycles_per_sample;
            self.generate_sample();
        }
    }

    fn clock_frame_seq(&mut self) {
        match self.frame_seq {
            0 | 4 => self.clock_lengths(),
            2 | 6 => {
                self.clock_lengths();
                self.ch1.clock_sweep();
            }
            7 => {
                self.ch1.env.clock();
                self.ch2.env.clock();
                self.ch4.env.clock();
            }
            _ => {}
        }
        self.frame_seq = (self.frame_seq + 1) & 7;
    }

    fn clock_lengths(&mut self) {
        self.ch1.clock_length();
        self.ch2.clock_length();
        self.ch3.clock_length();
        self.ch4.clock_length();
    }

    fn generate_sample(&mut self) {
        // Cap the buffer so unread audio (turbo/pause) can't grow unbounded.
        if self.samples.len() >= SAMPLE_RATE as usize {
            return;
        }
        let outs = [
            self.ch1.output(),
            self.ch2.output(),
            self.ch3.output(),
            self.ch4.output(),
        ];
        let dacs = [
            self.ch1.env.dac_on() && self.ch1.enabled,
            self.ch2.env.dac_on() && self.ch2.enabled,
            self.ch3.dac,
            self.ch4.env.dac_on() && self.ch4.enabled,
        ];
        let mut left = 0.0f32;
        let mut right = 0.0f32;
        for i in 0..4 {
            let v = if dacs[i] { outs[i] as f32 / 7.5 - 1.0 } else { 0.0 };
            if self.nr51 & (0x10 << i) != 0 {
                left += v;
            }
            if self.nr51 & (0x01 << i) != 0 {
                right += v;
            }
        }
        let lvol = ((self.nr50 >> 4) & 7) as f32 + 1.0;
        let rvol = (self.nr50 & 7) as f32 + 1.0;
        left = left / 4.0 * (lvol / 8.0);
        right = right / 4.0 * (rvol / 8.0);

        // High-pass "capacitor" filter, removes DC offset.
        let out_l = left - self.cap_l;
        self.cap_l = left - out_l * 0.9986;
        let out_r = right - self.cap_r;
        self.cap_r = right - out_r * 0.9986;

        self.samples.push(out_l);
        self.samples.push(out_r);
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0xFF10 => {
                0x80 | (self.ch1.sweep_period << 4)
                    | (self.ch1.sweep_negate as u8) << 3
                    | self.ch1.sweep_shift
            }
            0xFF11 => (self.ch1.duty << 6) | 0x3F,
            0xFF12 => self.ch1.env.read(),
            0xFF13 => 0xFF,
            0xFF14 => 0xBF | (self.ch1.length_enable as u8) << 6,
            0xFF16 => (self.ch2.duty << 6) | 0x3F,
            0xFF17 => self.ch2.env.read(),
            0xFF18 => 0xFF,
            0xFF19 => 0xBF | (self.ch2.length_enable as u8) << 6,
            0xFF1A => 0x7F | (self.ch3.dac as u8) << 7,
            0xFF1B => 0xFF,
            0xFF1C => 0x9F | (self.ch3.volume_shift << 5),
            0xFF1D => 0xFF,
            0xFF1E => 0xBF | (self.ch3.length_enable as u8) << 6,
            0xFF20 => 0xFF,
            0xFF21 => self.ch4.env.read(),
            0xFF22 => (self.ch4.shift << 4) | (self.ch4.width7 as u8) << 3 | self.ch4.divisor,
            0xFF23 => 0xBF | (self.ch4.length_enable as u8) << 6,
            0xFF24 => self.nr50,
            0xFF25 => self.nr51,
            0xFF26 => {
                0x70 | (self.enabled as u8) << 7
                    | (self.ch4.enabled as u8) << 3
                    | (self.ch3.enabled as u8) << 2
                    | (self.ch2.enabled as u8) << 1
                    | self.ch1.enabled as u8
            }
            0xFF30..=0xFF3F => self.wave_ram[(addr - 0xFF30) as usize],
            _ => 0xFF,
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        if !self.enabled && addr != 0xFF26 && !(0xFF30..=0xFF3F).contains(&addr) {
            return;
        }
        match addr {
            0xFF10 => {
                let old_negate = self.ch1.sweep_negate;
                self.ch1.sweep_period = (val >> 4) & 7;
                self.ch1.sweep_negate = val & 0x08 != 0;
                self.ch1.sweep_shift = val & 7;
                // Clearing negate after it was used disables the channel.
                if old_negate && !self.ch1.sweep_negate && self.ch1.sweep_negate_used {
                    self.ch1.enabled = false;
                }
            }
            0xFF11 => {
                self.ch1.duty = val >> 6;
                self.ch1.length = 64 - (val & 0x3F) as u16;
            }
            0xFF12 => {
                self.ch1.env.write(val);
                if !self.ch1.env.dac_on() {
                    self.ch1.enabled = false;
                }
            }
            0xFF13 => self.ch1.freq = (self.ch1.freq & 0x700) | val as u16,
            0xFF14 => {
                self.ch1.freq = (self.ch1.freq & 0xFF) | ((val as u16 & 7) << 8);
                self.ch1.length_enable = val & 0x40 != 0;
                if val & 0x80 != 0 {
                    self.ch1.trigger(true);
                }
            }
            0xFF16 => {
                self.ch2.duty = val >> 6;
                self.ch2.length = 64 - (val & 0x3F) as u16;
            }
            0xFF17 => {
                self.ch2.env.write(val);
                if !self.ch2.env.dac_on() {
                    self.ch2.enabled = false;
                }
            }
            0xFF18 => self.ch2.freq = (self.ch2.freq & 0x700) | val as u16,
            0xFF19 => {
                self.ch2.freq = (self.ch2.freq & 0xFF) | ((val as u16 & 7) << 8);
                self.ch2.length_enable = val & 0x40 != 0;
                if val & 0x80 != 0 {
                    self.ch2.trigger(false);
                }
            }
            0xFF1A => {
                self.ch3.dac = val & 0x80 != 0;
                if !self.ch3.dac {
                    self.ch3.enabled = false;
                }
            }
            0xFF1B => self.ch3.length = 256 - val as u16,
            0xFF1C => self.ch3.volume_shift = (val >> 5) & 3,
            0xFF1D => self.ch3.freq = (self.ch3.freq & 0x700) | val as u16,
            0xFF1E => {
                self.ch3.freq = (self.ch3.freq & 0xFF) | ((val as u16 & 7) << 8);
                self.ch3.length_enable = val & 0x40 != 0;
                if val & 0x80 != 0 {
                    self.ch3.enabled = self.ch3.dac;
                    if self.ch3.length == 0 {
                        self.ch3.length = 256;
                    }
                    self.ch3.timer = ((2048 - self.ch3.freq as i32) * 2).max(2);
                    self.ch3.pos = 0;
                }
            }
            0xFF20 => self.ch4.length = 64 - (val & 0x3F) as u16,
            0xFF21 => {
                self.ch4.env.write(val);
                if !self.ch4.env.dac_on() {
                    self.ch4.enabled = false;
                }
            }
            0xFF22 => {
                self.ch4.shift = val >> 4;
                self.ch4.width7 = val & 0x08 != 0;
                self.ch4.divisor = val & 7;
            }
            0xFF23 => {
                self.ch4.length_enable = val & 0x40 != 0;
                if val & 0x80 != 0 {
                    self.ch4.enabled = self.ch4.env.dac_on();
                    if self.ch4.length == 0 {
                        self.ch4.length = 64;
                    }
                    self.ch4.timer = self.ch4.period().max(8);
                    self.ch4.lfsr = 0x7FFF;
                    self.ch4.env.trigger();
                }
            }
            0xFF24 => self.nr50 = val,
            0xFF25 => self.nr51 = val,
            0xFF26 => {
                let on = val & 0x80 != 0;
                if self.enabled && !on {
                    // Power off clears all registers.
                    let wave = self.wave_ram;
                    *self = Apu {
                        samples: std::mem::take(&mut self.samples),
                        sample_timer: self.sample_timer,
                        cap_l: self.cap_l,
                        cap_r: self.cap_r,
                        ..Apu::new()
                    };
                    self.wave_ram = wave;
                    self.enabled = false;
                    self.nr50 = 0;
                    self.nr51 = 0;
                } else if !self.enabled && on {
                    self.enabled = true;
                    self.frame_seq = 0;
                }
            }
            0xFF30..=0xFF3F => self.wave_ram[(addr - 0xFF30) as usize] = val,
            _ => {}
        }
    }
}
