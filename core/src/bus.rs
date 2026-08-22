// System bus: memory map, interrupt flags, OAM DMA, CGB HDMA/GDMA,
// WRAM/VRAM banking and the double-speed switch.

use crate::apu::Apu;
use crate::cartridge::Cartridge;
use crate::joypad::Joypad;
use crate::ppu::Ppu;
use crate::timer::Timer;

pub const INT_VBLANK: u8 = 0x01;
pub const INT_STAT: u8 = 0x02;
pub const INT_TIMER: u8 = 0x04;
pub const INT_SERIAL: u8 = 0x08;
pub const INT_JOYPAD: u8 = 0x10;

pub struct Bus {
    pub cart: Cartridge,
    pub ppu: Ppu,
    pub apu: Apu,
    pub timer: Timer,
    pub joypad: Joypad,

    pub cgb: bool,
    pub(crate) wram: [u8; 0x8000], // 8 banks of 4 KiB
    pub(crate) wram_bank: usize,
    pub(crate) hram: [u8; 0x7F],

    pub ie: u8,
    pub iff: u8,

    // Serial (stubbed link cable; also captures output for test ROMs)
    pub(crate) sb: u8,
    pub(crate) sc: u8,
    pub(crate) serial_counter: i32,
    pub serial_out: Vec<u8>,

    // CGB speed switch
    pub double_speed: bool,
    pub(crate) key1_armed: bool,

    // HDMA
    pub(crate) hdma_src: u16,
    pub(crate) hdma_dst: u16,
    pub(crate) hdma_len: u8, // remaining blocks - 1; 0xFF = inactive
    pub(crate) hdma_active: bool,

    /// Real-time (4.19 MHz) cycles elapsed — drives frame pacing.
    pub frame_cycles: u64,
}

impl Bus {
    pub fn new(cart: Cartridge) -> Self {
        let cgb = cart.cgb;
        Bus {
            cart,
            ppu: Ppu::new(cgb),
            apu: Apu::new(),
            timer: Timer::new(),
            joypad: Joypad::new(),
            cgb,
            wram: [0; 0x8000],
            wram_bank: 1,
            hram: [0; 0x7F],
            ie: 0,
            iff: 0xE1,
            sb: 0,
            sc: 0x7E,
            serial_counter: 0,
            serial_out: Vec::new(),
            double_speed: false,
            key1_armed: false,
            hdma_src: 0,
            hdma_dst: 0,
            hdma_len: 0xFF,
            hdma_active: false,
            frame_cycles: 0,
        }
    }

    /// Advance all subsystems by `cycles` CPU T-cycles.
    pub fn tick(&mut self, cycles: u32) {
        // Timer and serial run at CPU speed; PPU/APU at real (4.19 MHz) speed.
        self.timer.tick(cycles);
        if self.timer.interrupt {
            self.timer.interrupt = false;
            self.iff |= INT_TIMER;
        }

        if self.sc & 0x80 != 0 && self.sc & 0x01 != 0 {
            self.serial_counter -= cycles as i32;
            if self.serial_counter <= 0 {
                // No link partner: shift in 0xFF.
                self.serial_out.push(self.sb);
                self.sb = 0xFF;
                self.sc &= 0x7F;
                self.iff |= INT_SERIAL;
            }
        }

        let real = if self.double_speed { cycles / 2 } else { cycles };
        self.frame_cycles += real as u64;

        self.ppu.tick(real);
        if self.ppu.int_vblank {
            self.ppu.int_vblank = false;
            self.iff |= INT_VBLANK;
        }
        if self.ppu.int_stat {
            self.ppu.int_stat = false;
            self.iff |= INT_STAT;
        }
        if self.ppu.entered_hblank {
            self.ppu.entered_hblank = false;
            if self.hdma_active {
                self.hdma_block();
            }
        }

        self.apu.tick(real);

        if self.joypad.interrupt {
            self.joypad.interrupt = false;
            self.iff |= INT_JOYPAD;
        }
    }

    pub fn speed_switch_armed(&self) -> bool {
        self.cgb && self.key1_armed
    }

    pub fn perform_speed_switch(&mut self) {
        self.double_speed = !self.double_speed;
        self.key1_armed = false;
        self.timer.reset_div();
    }

    pub fn read(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x7FFF => self.cart.read_rom(addr),
            0x8000..=0x9FFF => self.ppu.read_vram(addr),
            0xA000..=0xBFFF => self.cart.read_ram(addr & 0x1FFF),
            0xC000..=0xCFFF => self.wram[addr as usize - 0xC000],
            0xD000..=0xDFFF => self.wram[self.wram_bank * 0x1000 + addr as usize - 0xD000],
            0xE000..=0xFDFF => self.read(addr - 0x2000),
            0xFE00..=0xFE9F => self.ppu.oam[addr as usize - 0xFE00],
            0xFEA0..=0xFEFF => 0xFF,
            0xFF00..=0xFF7F => self.read_io(addr),
            0xFF80..=0xFFFE => self.hram[addr as usize - 0xFF80],
            0xFFFF => self.ie,
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0x0000..=0x7FFF => self.cart.write_rom(addr, val),
            0x8000..=0x9FFF => self.ppu.write_vram(addr, val),
            0xA000..=0xBFFF => self.cart.write_ram(addr & 0x1FFF, val),
            0xC000..=0xCFFF => self.wram[addr as usize - 0xC000] = val,
            0xD000..=0xDFFF => self.wram[self.wram_bank * 0x1000 + addr as usize - 0xD000] = val,
            0xE000..=0xFDFF => self.write(addr - 0x2000, val),
            0xFE00..=0xFE9F => self.ppu.oam[addr as usize - 0xFE00] = val,
            0xFEA0..=0xFEFF => {}
            0xFF00..=0xFF7F => self.write_io(addr, val),
            0xFF80..=0xFFFE => self.hram[addr as usize - 0xFF80] = val,
            0xFFFF => self.ie = val,
        }
    }

    fn read_io(&mut self, addr: u16) -> u8 {
        match addr {
            0xFF00 => self.joypad.read(),
            0xFF01 => self.sb,
            0xFF02 => self.sc | if self.cgb { 0x7C } else { 0x7E },
            0xFF04..=0xFF07 => self.timer.read(addr),
            0xFF0F => self.iff | 0xE0,
            0xFF10..=0xFF3F => self.apu.read(addr),
            0xFF40..=0xFF4B | 0xFF4F | 0xFF68..=0xFF6C => self.ppu.read_reg(addr),
            0xFF4D => {
                if self.cgb {
                    0x7E | (self.double_speed as u8) << 7 | self.key1_armed as u8
                } else {
                    0xFF
                }
            }
            0xFF55 => {
                if self.hdma_active {
                    self.hdma_len
                } else {
                    0x80 | self.hdma_len
                }
            }
            0xFF70 => {
                if self.cgb {
                    0xF8 | self.wram_bank as u8
                } else {
                    0xFF
                }
            }
            _ => 0xFF,
        }
    }

    fn write_io(&mut self, addr: u16, val: u8) {
        match addr {
            0xFF00 => self.joypad.write(val),
            0xFF01 => self.sb = val,
            0xFF02 => {
                self.sc = val & 0x83;
                if val & 0x80 != 0 && val & 0x01 != 0 {
                    self.serial_counter = 8 * 512; // 8 bits at 8192 Hz
                }
            }
            0xFF04..=0xFF07 => self.timer.write(addr, val),
            0xFF0F => self.iff = val & 0x1F,
            0xFF10..=0xFF3F => self.apu.write(addr, val),
            0xFF46 => self.oam_dma(val),
            0xFF40..=0xFF45 | 0xFF47..=0xFF4B | 0xFF4F | 0xFF68..=0xFF6C => {
                self.ppu.write_reg(addr, val)
            }
            0xFF4D => {
                if self.cgb {
                    self.key1_armed = val & 1 != 0;
                }
            }
            0xFF51 => self.hdma_src = (self.hdma_src & 0x00FF) | ((val as u16) << 8),
            0xFF52 => self.hdma_src = (self.hdma_src & 0xFF00) | (val & 0xF0) as u16,
            0xFF53 => self.hdma_dst = (self.hdma_dst & 0x00FF) | ((val as u16 & 0x1F) << 8),
            0xFF54 => self.hdma_dst = (self.hdma_dst & 0xFF00) | (val & 0xF0) as u16,
            0xFF55 => {
                if !self.cgb {
                    return;
                }
                if self.hdma_active {
                    // Writing with bit 7 clear cancels an active HBlank DMA.
                    if val & 0x80 == 0 {
                        self.hdma_active = false;
                    }
                    return;
                }
                self.hdma_len = val & 0x7F;
                if val & 0x80 != 0 {
                    self.hdma_active = true; // HBlank DMA: one block per HBlank
                } else {
                    // General-purpose DMA: transfer everything now.
                    let blocks = (val & 0x7F) as u16 + 1;
                    for _ in 0..blocks {
                        self.hdma_copy_block();
                    }
                    self.hdma_len = 0xFF;
                }
            }
            0xFF70 => {
                if self.cgb {
                    self.wram_bank = (val & 7).max(1) as usize;
                }
            }
            _ => {}
        }
    }

    fn oam_dma(&mut self, page: u8) {
        // Instant OAM DMA — games busy-wait the proper 160 µs in HRAM anyway.
        let base = (page as u16) << 8;
        for i in 0..0xA0 {
            let v = self.read(base + i);
            self.ppu.oam[i as usize] = v;
        }
    }

    fn hdma_copy_block(&mut self) {
        for _ in 0..16 {
            let v = self.read(self.hdma_src);
            let dst = 0x8000 | (self.hdma_dst & 0x1FFF);
            self.ppu.write_vram(dst, v);
            self.hdma_src = self.hdma_src.wrapping_add(1);
            self.hdma_dst = (self.hdma_dst.wrapping_add(1)) & 0x1FFF;
        }
    }

    fn hdma_block(&mut self) {
        self.hdma_copy_block();
        if self.hdma_len == 0 {
            self.hdma_active = false;
            self.hdma_len = 0xFF;
        } else {
            self.hdma_len -= 1;
        }
    }
}
