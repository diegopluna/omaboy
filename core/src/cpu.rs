// SM83 (Game Boy CPU) interpreter.
//
// Timing model: every memory access ticks the bus by one M-cycle (4 T-cycles),
// and instructions with internal cycles tick explicitly. This yields correct
// per-instruction timing and sub-instruction hardware synchronisation.

use crate::bus::Bus;

const FZ: u8 = 0x80;
const FN: u8 = 0x40;
const FH: u8 = 0x20;
const FC: u8 = 0x10;

pub struct Cpu {
    pub a: u8,
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub sp: u16,
    pub pc: u16,
    pub ime: bool,
    pub(crate) ime_pending: bool,
    pub halted: bool,
    pub(crate) halt_bug: bool,
}

impl Cpu {
    pub fn new(cgb: bool) -> Self {
        // Post-boot-ROM register state. A=0x11 tells games they run on a CGB.
        let mut cpu = Cpu {
            a: if cgb { 0x11 } else { 0x01 },
            f: 0xB0,
            b: 0x00,
            c: 0x13,
            d: 0x00,
            e: 0xD8,
            h: 0x01,
            l: 0x4D,
            sp: 0xFFFE,
            pc: 0x0100,
            ime: false,
            ime_pending: false,
            halted: false,
            halt_bug: false,
        };
        if cgb {
            cpu.f = 0x80;
            cpu.b = 0x00;
            cpu.c = 0x00;
            cpu.d = 0xFF;
            cpu.e = 0x56;
            cpu.h = 0x00;
            cpu.l = 0x0D;
        }
        cpu
    }

    #[inline]
    fn read(&mut self, bus: &mut Bus, addr: u16) -> u8 {
        bus.tick(4);
        bus.read(addr)
    }

    #[inline]
    fn write(&mut self, bus: &mut Bus, addr: u16, val: u8) {
        bus.tick(4);
        bus.write(addr, val);
    }

    #[inline]
    fn fetch(&mut self, bus: &mut Bus) -> u8 {
        let v = self.read(bus, self.pc);
        if self.halt_bug {
            self.halt_bug = false; // PC fails to increment once after the halt bug
        } else {
            self.pc = self.pc.wrapping_add(1);
        }
        v
    }

    #[inline]
    fn fetch16(&mut self, bus: &mut Bus) -> u16 {
        let lo = self.fetch(bus) as u16;
        let hi = self.fetch(bus) as u16;
        (hi << 8) | lo
    }

    #[inline]
    fn af(&self) -> u16 {
        ((self.a as u16) << 8) | self.f as u16
    }
    #[inline]
    fn bc(&self) -> u16 {
        ((self.b as u16) << 8) | self.c as u16
    }
    #[inline]
    fn de(&self) -> u16 {
        ((self.d as u16) << 8) | self.e as u16
    }
    #[inline]
    fn hl(&self) -> u16 {
        ((self.h as u16) << 8) | self.l as u16
    }
    #[inline]
    fn set_af(&mut self, v: u16) {
        self.a = (v >> 8) as u8;
        self.f = (v & 0xF0) as u8;
    }
    #[inline]
    fn set_bc(&mut self, v: u16) {
        self.b = (v >> 8) as u8;
        self.c = v as u8;
    }
    #[inline]
    fn set_de(&mut self, v: u16) {
        self.d = (v >> 8) as u8;
        self.e = v as u8;
    }
    #[inline]
    fn set_hl(&mut self, v: u16) {
        self.h = (v >> 8) as u8;
        self.l = v as u8;
    }

    #[inline]
    fn flag(&self, m: u8) -> bool {
        self.f & m != 0
    }
    #[inline]
    fn set_flags(&mut self, z: bool, n: bool, h: bool, c: bool) {
        self.f = (z as u8) << 7 | (n as u8) << 6 | (h as u8) << 5 | (c as u8) << 4;
    }

    // r8 index: 0=B 1=C 2=D 3=E 4=H 5=L 6=(HL) 7=A
    fn r8_get(&mut self, bus: &mut Bus, idx: u8) -> u8 {
        match idx {
            0 => self.b,
            1 => self.c,
            2 => self.d,
            3 => self.e,
            4 => self.h,
            5 => self.l,
            6 => {
                let hl = self.hl();
                self.read(bus, hl)
            }
            _ => self.a,
        }
    }

    fn r8_set(&mut self, bus: &mut Bus, idx: u8, val: u8) {
        match idx {
            0 => self.b = val,
            1 => self.c = val,
            2 => self.d = val,
            3 => self.e = val,
            4 => self.h = val,
            5 => self.l = val,
            6 => {
                let hl = self.hl();
                self.write(bus, hl, val)
            }
            _ => self.a = val,
        }
    }

    fn push16(&mut self, bus: &mut Bus, v: u16) {
        self.sp = self.sp.wrapping_sub(1);
        self.write(bus, self.sp, (v >> 8) as u8);
        self.sp = self.sp.wrapping_sub(1);
        self.write(bus, self.sp, v as u8);
    }

    fn pop16(&mut self, bus: &mut Bus) -> u16 {
        let lo = self.read(bus, self.sp) as u16;
        self.sp = self.sp.wrapping_add(1);
        let hi = self.read(bus, self.sp) as u16;
        self.sp = self.sp.wrapping_add(1);
        (hi << 8) | lo
    }

    /// Execute one instruction (or service an interrupt / idle in halt).
    pub fn step(&mut self, bus: &mut Bus) {
        // EI takes effect after the following instruction.
        let enable_ime_after = self.ime_pending;

        let pending = bus.ie & bus.iff & 0x1F;

        if self.halted {
            if pending != 0 {
                self.halted = false;
            } else {
                bus.tick(4);
                return;
            }
        }

        if self.ime && pending != 0 {
            self.service_interrupt(bus, pending);
            return;
        }

        let op = self.fetch(bus);
        self.execute(bus, op);

        if enable_ime_after && self.ime_pending {
            self.ime = true;
            self.ime_pending = false;
        }
    }

    fn service_interrupt(&mut self, bus: &mut Bus, pending: u8) {
        self.ime = false;
        self.ime_pending = false;
        bus.tick(8); // two internal cycles
        let bit = pending.trailing_zeros() as u8;
        bus.iff &= !(1 << bit);
        self.push16(bus, self.pc);
        self.pc = 0x0040 + (bit as u16) * 8;
        bus.tick(4);
    }

    // ---- ALU helpers ----

    fn alu_add(&mut self, v: u8, carry: bool) {
        let c = (carry && self.flag(FC)) as u8;
        let a = self.a;
        let r = a.wrapping_add(v).wrapping_add(c);
        self.set_flags(
            r == 0,
            false,
            (a & 0x0F) + (v & 0x0F) + c > 0x0F,
            (a as u16) + (v as u16) + (c as u16) > 0xFF,
        );
        self.a = r;
    }

    fn alu_sub(&mut self, v: u8, carry: bool, store: bool) {
        let c = (carry && self.flag(FC)) as u8;
        let a = self.a;
        let r = a.wrapping_sub(v).wrapping_sub(c);
        self.set_flags(
            r == 0,
            true,
            (a & 0x0F) < (v & 0x0F) + c,
            (a as u16) < (v as u16) + (c as u16),
        );
        if store {
            self.a = r;
        }
    }

    fn alu_and(&mut self, v: u8) {
        self.a &= v;
        self.set_flags(self.a == 0, false, true, false);
    }
    fn alu_xor(&mut self, v: u8) {
        self.a ^= v;
        self.set_flags(self.a == 0, false, false, false);
    }
    fn alu_or(&mut self, v: u8) {
        self.a |= v;
        self.set_flags(self.a == 0, false, false, false);
    }

    fn alu_inc(&mut self, v: u8) -> u8 {
        let r = v.wrapping_add(1);
        self.f = (self.f & FC) | if r == 0 { FZ } else { 0 } | if v & 0x0F == 0x0F { FH } else { 0 };
        r
    }

    fn alu_dec(&mut self, v: u8) -> u8 {
        let r = v.wrapping_sub(1);
        self.f = (self.f & FC) | FN | if r == 0 { FZ } else { 0 } | if v & 0x0F == 0 { FH } else { 0 };
        r
    }

    fn add_hl(&mut self, v: u16) {
        let hl = self.hl();
        let r = hl.wrapping_add(v);
        self.f = (self.f & FZ)
            | if (hl & 0x0FFF) + (v & 0x0FFF) > 0x0FFF { FH } else { 0 }
            | if (hl as u32) + (v as u32) > 0xFFFF { FC } else { 0 };
        self.set_hl(r);
    }

    fn add_sp_e(&mut self, bus: &mut Bus) -> u16 {
        let e = self.fetch(bus) as i8 as i16 as u16;
        let sp = self.sp;
        self.set_flags(
            false,
            false,
            (sp & 0x0F) + (e & 0x0F) > 0x0F,
            (sp & 0xFF) + (e & 0xFF) > 0xFF,
        );
        sp.wrapping_add(e)
    }

    fn daa(&mut self) {
        let mut a = self.a;
        let mut carry = self.flag(FC);
        if !self.flag(FN) {
            if carry || a > 0x99 {
                a = a.wrapping_add(0x60);
                carry = true;
            }
            if self.flag(FH) || (a & 0x0F) > 0x09 {
                a = a.wrapping_add(0x06);
            }
        } else {
            if carry {
                a = a.wrapping_sub(0x60);
            }
            if self.flag(FH) {
                a = a.wrapping_sub(0x06);
            }
        }
        self.f = (self.f & (FN | FC)) | if a == 0 { FZ } else { 0 } | if carry { FC } else { 0 };
        self.a = a;
    }

    // ---- rotates/shifts (CB and A-register variants) ----

    fn rlc(&mut self, v: u8) -> u8 {
        let r = v.rotate_left(1);
        self.set_flags(r == 0, false, false, v & 0x80 != 0);
        r
    }
    fn rrc(&mut self, v: u8) -> u8 {
        let r = v.rotate_right(1);
        self.set_flags(r == 0, false, false, v & 1 != 0);
        r
    }
    fn rl(&mut self, v: u8) -> u8 {
        let r = (v << 1) | self.flag(FC) as u8;
        self.set_flags(r == 0, false, false, v & 0x80 != 0);
        r
    }
    fn rr(&mut self, v: u8) -> u8 {
        let r = (v >> 1) | ((self.flag(FC) as u8) << 7);
        self.set_flags(r == 0, false, false, v & 1 != 0);
        r
    }
    fn sla(&mut self, v: u8) -> u8 {
        let r = v << 1;
        self.set_flags(r == 0, false, false, v & 0x80 != 0);
        r
    }
    fn sra(&mut self, v: u8) -> u8 {
        let r = (v >> 1) | (v & 0x80);
        self.set_flags(r == 0, false, false, v & 1 != 0);
        r
    }
    fn swap(&mut self, v: u8) -> u8 {
        let r = v.rotate_left(4);
        self.set_flags(r == 0, false, false, false);
        r
    }
    fn srl(&mut self, v: u8) -> u8 {
        let r = v >> 1;
        self.set_flags(r == 0, false, false, v & 1 != 0);
        r
    }

    #[inline]
    fn cond(&self, idx: u8) -> bool {
        match idx {
            0 => !self.flag(FZ),
            1 => self.flag(FZ),
            2 => !self.flag(FC),
            _ => self.flag(FC),
        }
    }

    fn execute(&mut self, bus: &mut Bus, op: u8) {
        match op {
            0x00 => {} // NOP
            0x10 => {
                // STOP: on CGB, performs the speed switch if armed.
                self.fetch(bus);
                if bus.speed_switch_armed() {
                    bus.perform_speed_switch();
                }
            }
            0x76 => {
                // HALT
                let pending = bus.ie & bus.iff & 0x1F;
                if !self.ime && pending != 0 {
                    self.halt_bug = true;
                } else {
                    self.halted = true;
                }
            }
            0xF3 => {
                self.ime = false;
                self.ime_pending = false;
            }
            0xFB => {
                if !self.ime {
                    self.ime_pending = true;
                }
            }

            // 16-bit loads
            0x01 => {
                let v = self.fetch16(bus);
                self.set_bc(v);
            }
            0x11 => {
                let v = self.fetch16(bus);
                self.set_de(v);
            }
            0x21 => {
                let v = self.fetch16(bus);
                self.set_hl(v);
            }
            0x31 => {
                self.sp = self.fetch16(bus);
            }
            0x08 => {
                let addr = self.fetch16(bus);
                let sp = self.sp;
                self.write(bus, addr, sp as u8);
                self.write(bus, addr.wrapping_add(1), (sp >> 8) as u8);
            }
            0xF9 => {
                self.sp = self.hl();
                bus.tick(4);
            }
            0xF8 => {
                let r = self.add_sp_e(bus);
                self.set_hl(r);
                bus.tick(4);
            }
            0xE8 => {
                self.sp = self.add_sp_e(bus);
                bus.tick(8);
            }

            // (rr) <-> A
            0x02 => {
                let a = self.a;
                let addr = self.bc();
                self.write(bus, addr, a);
            }
            0x12 => {
                let a = self.a;
                let addr = self.de();
                self.write(bus, addr, a);
            }
            0x22 => {
                let a = self.a;
                let hl = self.hl();
                self.write(bus, hl, a);
                self.set_hl(hl.wrapping_add(1));
            }
            0x32 => {
                let a = self.a;
                let hl = self.hl();
                self.write(bus, hl, a);
                self.set_hl(hl.wrapping_sub(1));
            }
            0x0A => {
                let addr = self.bc();
                self.a = self.read(bus, addr);
            }
            0x1A => {
                let addr = self.de();
                self.a = self.read(bus, addr);
            }
            0x2A => {
                let hl = self.hl();
                self.a = self.read(bus, hl);
                self.set_hl(hl.wrapping_add(1));
            }
            0x3A => {
                let hl = self.hl();
                self.a = self.read(bus, hl);
                self.set_hl(hl.wrapping_sub(1));
            }

            // INC/DEC rr (internal cycle)
            0x03 => {
                let v = self.bc().wrapping_add(1);
                self.set_bc(v);
                bus.tick(4);
            }
            0x13 => {
                let v = self.de().wrapping_add(1);
                self.set_de(v);
                bus.tick(4);
            }
            0x23 => {
                let v = self.hl().wrapping_add(1);
                self.set_hl(v);
                bus.tick(4);
            }
            0x33 => {
                self.sp = self.sp.wrapping_add(1);
                bus.tick(4);
            }
            0x0B => {
                let v = self.bc().wrapping_sub(1);
                self.set_bc(v);
                bus.tick(4);
            }
            0x1B => {
                let v = self.de().wrapping_sub(1);
                self.set_de(v);
                bus.tick(4);
            }
            0x2B => {
                let v = self.hl().wrapping_sub(1);
                self.set_hl(v);
                bus.tick(4);
            }
            0x3B => {
                self.sp = self.sp.wrapping_sub(1);
                bus.tick(4);
            }

            // ADD HL,rr
            0x09 => {
                let v = self.bc();
                self.add_hl(v);
                bus.tick(4);
            }
            0x19 => {
                let v = self.de();
                self.add_hl(v);
                bus.tick(4);
            }
            0x29 => {
                let v = self.hl();
                self.add_hl(v);
                bus.tick(4);
            }
            0x39 => {
                let v = self.sp;
                self.add_hl(v);
                bus.tick(4);
            }

            // INC/DEC r8: xx000100 / xx000101 pattern within 0x04..=0x3D
            0x04 | 0x0C | 0x14 | 0x1C | 0x24 | 0x2C | 0x34 | 0x3C => {
                let idx = (op >> 3) & 7;
                let v = self.r8_get(bus, idx);
                let r = self.alu_inc(v);
                self.r8_set(bus, idx, r);
            }
            0x05 | 0x0D | 0x15 | 0x1D | 0x25 | 0x2D | 0x35 | 0x3D => {
                let idx = (op >> 3) & 7;
                let v = self.r8_get(bus, idx);
                let r = self.alu_dec(v);
                self.r8_set(bus, idx, r);
            }

            // LD r8, d8
            0x06 | 0x0E | 0x16 | 0x1E | 0x26 | 0x2E | 0x36 | 0x3E => {
                let idx = (op >> 3) & 7;
                let v = self.fetch(bus);
                self.r8_set(bus, idx, v);
            }

            // Accumulator rotates (flags: Z always cleared)
            0x07 => {
                let a = self.a;
                self.a = self.rlc(a);
                self.f &= !FZ;
            }
            0x0F => {
                let a = self.a;
                self.a = self.rrc(a);
                self.f &= !FZ;
            }
            0x17 => {
                let a = self.a;
                self.a = self.rl(a);
                self.f &= !FZ;
            }
            0x1F => {
                let a = self.a;
                self.a = self.rr(a);
                self.f &= !FZ;
            }

            0x27 => self.daa(),
            0x2F => {
                self.a = !self.a;
                self.f |= FN | FH;
            }
            0x37 => {
                self.f = (self.f & FZ) | FC;
            }
            0x3F => {
                self.f = (self.f & (FZ | FC)) ^ FC;
            }

            // JR
            0x18 => {
                let e = self.fetch(bus) as i8;
                self.pc = self.pc.wrapping_add(e as u16);
                bus.tick(4);
            }
            0x20 | 0x28 | 0x30 | 0x38 => {
                let e = self.fetch(bus) as i8;
                if self.cond((op >> 3) & 3) {
                    self.pc = self.pc.wrapping_add(e as u16);
                    bus.tick(4);
                }
            }

            // LD r8, r8 (0x40..=0x7F except 0x76 HALT handled above)
            0x40..=0x7F => {
                let src = op & 7;
                let dst = (op >> 3) & 7;
                let v = self.r8_get(bus, src);
                self.r8_set(bus, dst, v);
            }

            // ALU A, r8
            0x80..=0xBF => {
                let v = self.r8_get(bus, op & 7);
                match (op >> 3) & 7 {
                    0 => self.alu_add(v, false),
                    1 => self.alu_add(v, true),
                    2 => self.alu_sub(v, false, true),
                    3 => self.alu_sub(v, true, true),
                    4 => self.alu_and(v),
                    5 => self.alu_xor(v),
                    6 => self.alu_or(v),
                    _ => self.alu_sub(v, false, false), // CP
                }
            }

            // ALU A, d8
            0xC6 | 0xCE | 0xD6 | 0xDE | 0xE6 | 0xEE | 0xF6 | 0xFE => {
                let v = self.fetch(bus);
                match (op >> 3) & 7 {
                    0 => self.alu_add(v, false),
                    1 => self.alu_add(v, true),
                    2 => self.alu_sub(v, false, true),
                    3 => self.alu_sub(v, true, true),
                    4 => self.alu_and(v),
                    5 => self.alu_xor(v),
                    6 => self.alu_or(v),
                    _ => self.alu_sub(v, false, false),
                }
            }

            // RET / RETI / RET cc
            0xC9 => {
                self.pc = self.pop16(bus);
                bus.tick(4);
            }
            0xD9 => {
                self.pc = self.pop16(bus);
                self.ime = true;
                bus.tick(4);
            }
            0xC0 | 0xC8 | 0xD0 | 0xD8 => {
                bus.tick(4);
                if self.cond((op >> 3) & 3) {
                    self.pc = self.pop16(bus);
                    bus.tick(4);
                }
            }

            // JP
            0xC3 => {
                self.pc = self.fetch16(bus);
                bus.tick(4);
            }
            0xE9 => {
                self.pc = self.hl();
            }
            0xC2 | 0xCA | 0xD2 | 0xDA => {
                let addr = self.fetch16(bus);
                if self.cond((op >> 3) & 3) {
                    self.pc = addr;
                    bus.tick(4);
                }
            }

            // CALL
            0xCD => {
                let addr = self.fetch16(bus);
                bus.tick(4);
                let pc = self.pc;
                self.push16(bus, pc);
                self.pc = addr;
            }
            0xC4 | 0xCC | 0xD4 | 0xDC => {
                let addr = self.fetch16(bus);
                if self.cond((op >> 3) & 3) {
                    bus.tick(4);
                    let pc = self.pc;
                    self.push16(bus, pc);
                    self.pc = addr;
                }
            }

            // RST
            0xC7 | 0xCF | 0xD7 | 0xDF | 0xE7 | 0xEF | 0xF7 | 0xFF => {
                bus.tick(4);
                let pc = self.pc;
                self.push16(bus, pc);
                self.pc = (op & 0x38) as u16;
            }

            // PUSH / POP
            0xC5 => {
                bus.tick(4);
                let v = self.bc();
                self.push16(bus, v);
            }
            0xD5 => {
                bus.tick(4);
                let v = self.de();
                self.push16(bus, v);
            }
            0xE5 => {
                bus.tick(4);
                let v = self.hl();
                self.push16(bus, v);
            }
            0xF5 => {
                bus.tick(4);
                let v = self.af();
                self.push16(bus, v);
            }
            0xC1 => {
                let v = self.pop16(bus);
                self.set_bc(v);
            }
            0xD1 => {
                let v = self.pop16(bus);
                self.set_de(v);
            }
            0xE1 => {
                let v = self.pop16(bus);
                self.set_hl(v);
            }
            0xF1 => {
                let v = self.pop16(bus);
                self.set_af(v);
            }

            // High-page and absolute loads
            0xE0 => {
                let off = self.fetch(bus) as u16;
                let a = self.a;
                self.write(bus, 0xFF00 | off, a);
            }
            0xF0 => {
                let off = self.fetch(bus) as u16;
                self.a = self.read(bus, 0xFF00 | off);
            }
            0xE2 => {
                let a = self.a;
                let addr = 0xFF00 | self.c as u16;
                self.write(bus, addr, a);
            }
            0xF2 => {
                let addr = 0xFF00 | self.c as u16;
                self.a = self.read(bus, addr);
            }
            0xEA => {
                let addr = self.fetch16(bus);
                let a = self.a;
                self.write(bus, addr, a);
            }
            0xFA => {
                let addr = self.fetch16(bus);
                self.a = self.read(bus, addr);
            }

            0xCB => {
                let cb = self.fetch(bus);
                self.execute_cb(bus, cb);
            }

            // Unused opcodes hard-lock real hardware; treat as NOP.
            0xD3 | 0xDB | 0xDD | 0xE3 | 0xE4 | 0xEB | 0xEC | 0xED | 0xF4 | 0xFC | 0xFD => {}
        }
    }

    fn execute_cb(&mut self, bus: &mut Bus, op: u8) {
        let idx = op & 7;
        match op >> 6 {
            0 => {
                let v = self.r8_get(bus, idx);
                let r = match (op >> 3) & 7 {
                    0 => self.rlc(v),
                    1 => self.rrc(v),
                    2 => self.rl(v),
                    3 => self.rr(v),
                    4 => self.sla(v),
                    5 => self.sra(v),
                    6 => self.swap(v),
                    _ => self.srl(v),
                };
                self.r8_set(bus, idx, r);
            }
            1 => {
                // BIT b, r8
                let bit = (op >> 3) & 7;
                let v = self.r8_get(bus, idx);
                self.f = (self.f & FC) | FH | if v & (1 << bit) == 0 { FZ } else { 0 };
            }
            2 => {
                // RES
                let bit = (op >> 3) & 7;
                let v = self.r8_get(bus, idx);
                self.r8_set(bus, idx, v & !(1 << bit));
            }
            _ => {
                // SET
                let bit = (op >> 3) & 7;
                let v = self.r8_get(bus, idx);
                self.r8_set(bus, idx, v | (1 << bit));
            }
        }
    }
}
