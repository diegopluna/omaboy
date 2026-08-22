use crate::bus::Bus;
use crate::cartridge::Cartridge;
use crate::cpu::Cpu;

pub struct Gameboy {
    pub cpu: Cpu,
    pub bus: Bus,
}

impl Gameboy {
    pub fn new(rom: Vec<u8>) -> Result<Self, String> {
        let cart = Cartridge::new(rom)?;
        let cgb = cart.cgb;
        Ok(Gameboy { cpu: Cpu::new(cgb), bus: Bus::new(cart) })
    }

    pub fn reset(&mut self) {
        let rom = std::mem::take(&mut self.bus.cart.rom);
        let ram = std::mem::take(&mut self.bus.cart.ram);
        let dmg_colors = self.bus.ppu.dmg_colors;
        let color_correction = self.bus.ppu.color_correction;
        if let Ok(mut fresh) = Gameboy::new(rom) {
            fresh.bus.cart.ram = ram;
            fresh.bus.ppu.dmg_colors = dmg_colors;
            fresh.bus.ppu.color_correction = color_correction;
            *self = fresh;
        }
    }

    /// Run until the PPU completes a frame (or a safety cycle cap, so a
    /// disabled LCD can't spin forever).
    pub fn run_frame(&mut self) {
        self.bus.ppu.frame_ready = false;
        let start = self.bus.frame_cycles;
        while !self.bus.ppu.frame_ready {
            self.cpu.step(&mut self.bus);
            if self.bus.frame_cycles - start > 80000 {
                break;
            }
        }
    }
}
