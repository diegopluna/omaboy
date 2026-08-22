// Joypad matrix. Button bit order (matches the FFI):
// 0=Right 1=Left 2=Up 3=Down 4=A 5=B 6=Select 7=Start

pub struct Joypad {
    pub(crate) select: u8, // P1 bits 4-5 (select lines, active low)
    pub buttons: u8, // 1 = pressed
    pub interrupt: bool,
}

impl Joypad {
    pub fn new() -> Self {
        Joypad { select: 0x30, buttons: 0, interrupt: false }
    }

    pub fn set_buttons(&mut self, buttons: u8) {
        // Interrupt on any newly pressed button.
        if buttons & !self.buttons != 0 {
            self.interrupt = true;
        }
        self.buttons = buttons;
    }

    pub fn read(&self) -> u8 {
        let mut lines = 0x0F;
        if self.select & 0x10 == 0 {
            lines &= !(self.buttons & 0x0F); // directions
        }
        if self.select & 0x20 == 0 {
            lines &= !(self.buttons >> 4); // action buttons
        }
        0xC0 | self.select | (lines & 0x0F)
    }

    pub fn write(&mut self, val: u8) {
        self.select = val & 0x30;
    }
}
