// DIV/TIMA timer with the internal 16-bit divider and falling-edge behaviour.

pub struct Timer {
    pub(crate) div: u16, // internal counter; DIV register is the high byte
    pub tima: u8,
    pub tma: u8,
    pub tac: u8,
    pub interrupt: bool,
}

impl Timer {
    pub fn new() -> Self {
        Timer { div: 0xABCC, tima: 0, tma: 0, tac: 0xF8, interrupt: false }
    }

    #[inline]
    fn tap(&self) -> bool {
        if self.tac & 0x04 == 0 {
            return false;
        }
        let bit = match self.tac & 3 {
            0 => 9,
            1 => 3,
            2 => 5,
            _ => 7,
        };
        self.div & (1 << bit) != 0
    }

    pub fn tick(&mut self, cycles: u32) {
        for _ in 0..cycles {
            let before = self.tap();
            self.div = self.div.wrapping_add(1);
            if before && !self.tap() {
                self.increment_tima();
            }
        }
    }

    fn increment_tima(&mut self) {
        let (v, overflow) = self.tima.overflowing_add(1);
        self.tima = if overflow { self.tma } else { v };
        if overflow {
            self.interrupt = true;
        }
    }

    pub fn read(&self, addr: u16) -> u8 {
        match addr {
            0xFF04 => (self.div >> 8) as u8,
            0xFF05 => self.tima,
            0xFF06 => self.tma,
            _ => self.tac | 0xF8,
        }
    }

    pub fn write(&mut self, addr: u16, val: u8) {
        match addr {
            0xFF04 => {
                // Resetting DIV can produce a falling edge on the tap.
                if self.tap() {
                    self.increment_tima();
                }
                self.div = 0;
            }
            0xFF05 => self.tima = val,
            0xFF06 => self.tma = val,
            _ => self.tac = val & 0x07,
        }
    }

    pub fn reset_div(&mut self) {
        self.div = 0;
    }
}
