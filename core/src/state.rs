// Save states: full machine snapshot (except APU internals, which games
// re-drive continuously; audio recovers within a note or two).

use crate::gameboy::Gameboy;

const MAGIC: &[u8; 4] = b"OMST";
const VERSION: u32 = 1;

struct W {
    buf: Vec<u8>,
}

impl W {
    fn new() -> Self {
        W { buf: Vec::with_capacity(256 * 1024) }
    }
    fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }
    fn b(&mut self, v: bool) {
        self.buf.push(v as u8);
    }
    fn u16(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    fn bytes(&mut self, v: &[u8]) {
        self.buf.extend_from_slice(v);
    }
}

struct R<'a> {
    d: &'a [u8],
    p: usize,
}

impl<'a> R<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let s = self.d.get(self.p..self.p + n)?;
        self.p += n;
        Some(s)
    }
    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
    fn b(&mut self) -> Option<bool> {
        Some(self.u8()? != 0)
    }
    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.take(2)?.try_into().ok()?))
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take(8)?.try_into().ok()?))
    }
    fn into_slice(&mut self, dst: &mut [u8]) -> Option<()> {
        let n = dst.len();
        dst.copy_from_slice(self.take(n)?);
        Some(())
    }
}

fn rtc_write(w: &mut W, r: &crate::cartridge::Rtc) {
    w.u8(r.seconds);
    w.u8(r.minutes);
    w.u8(r.hours);
    w.u16(r.days);
    w.b(r.halted);
    w.b(r.day_carry);
    w.u64(r.base);
}

fn rtc_read(r: &mut R) -> Option<crate::cartridge::Rtc> {
    Some(crate::cartridge::Rtc {
        seconds: r.u8()?,
        minutes: r.u8()?,
        hours: r.u8()?,
        days: r.u16()?,
        halted: r.b()?,
        day_carry: r.b()?,
        base: r.u64()?,
    })
}

impl Gameboy {
    /// ROM identity: title + checksums from the header.
    fn fingerprint(&self) -> [u8; 28] {
        let mut fp = [0u8; 28];
        fp.copy_from_slice(&self.bus.cart.rom[0x134..0x150]);
        fp
    }

    pub fn save_state(&self) -> Vec<u8> {
        let mut w = W::new();
        w.bytes(MAGIC);
        w.u32(VERSION);
        w.bytes(&self.fingerprint());

        let c = &self.cpu;
        for v in [c.a, c.f, c.b, c.c, c.d, c.e, c.h, c.l] {
            w.u8(v);
        }
        w.u16(c.sp);
        w.u16(c.pc);
        w.b(c.ime);
        w.b(c.ime_pending);
        w.b(c.halted);
        w.b(c.halt_bug);

        let b = &self.bus;
        w.bytes(&b.wram);
        w.u8(b.wram_bank as u8);
        w.bytes(&b.hram);
        w.u8(b.ie);
        w.u8(b.iff);
        w.u8(b.sb);
        w.u8(b.sc);
        w.u32(b.serial_counter as u32);
        w.b(b.double_speed);
        w.b(b.key1_armed);
        w.u16(b.hdma_src);
        w.u16(b.hdma_dst);
        w.u8(b.hdma_len);
        w.b(b.hdma_active);
        w.u64(b.frame_cycles);

        let t = &b.timer;
        w.u16(t.div);
        w.u8(t.tima);
        w.u8(t.tma);
        w.u8(t.tac);

        w.u8(b.joypad.select);

        let cart = &b.cart;
        w.u32(cart.ram.len() as u32);
        w.bytes(&cart.ram);
        w.b(cart.ram_enabled);
        w.u32(cart.rom_bank as u32);
        w.u32(cart.ram_bank as u32);
        w.u32(cart.bank_hi as u32);
        w.b(cart.mbc1_mode);
        rtc_write(&mut w, &cart.rtc);
        rtc_write(&mut w, &cart.rtc_latched);
        w.u8(cart.rtc_latch_state);

        let p = &b.ppu;
        w.bytes(&p.vram);
        w.u8(p.vram_bank as u8);
        w.bytes(&p.oam);
        for v in [
            p.lcdc, p.stat, p.scy, p.scx, p.ly, p.lyc, p.bgp, p.obp0, p.obp1, p.wy, p.wx,
            p.bcps, p.ocps,
        ] {
            w.u8(v);
        }
        w.b(p.opri);
        w.bytes(&p.bg_pal);
        w.bytes(&p.ob_pal);
        w.u8(p.mode);
        w.u32(p.dot);
        w.u16(p.window_line);
        w.b(p.stat_line);
        for px in p.framebuffer.iter() {
            w.u32(*px);
        }

        w.buf
    }

    pub fn load_state(&mut self, data: &[u8]) -> bool {
        // Snapshot first: a truncated file must not leave a half-applied state.
        let backup = self.save_state();
        if self.apply_state(data).is_some() {
            true
        } else {
            let _ = self.apply_state(&backup);
            false
        }
    }

    fn apply_state(&mut self, data: &[u8]) -> Option<()> {
        let mut r = R { d: data, p: 0 };
        if r.take(4)? != MAGIC || r.u32()? != VERSION {
            return None;
        }
        if r.take(28)? != self.fingerprint() {
            return None; // state belongs to a different game
        }

        let c = &mut self.cpu;
        c.a = r.u8()?;
        c.f = r.u8()?;
        c.b = r.u8()?;
        c.c = r.u8()?;
        c.d = r.u8()?;
        c.e = r.u8()?;
        c.h = r.u8()?;
        c.l = r.u8()?;
        c.sp = r.u16()?;
        c.pc = r.u16()?;
        c.ime = r.b()?;
        c.ime_pending = r.b()?;
        c.halted = r.b()?;
        c.halt_bug = r.b()?;

        let b = &mut self.bus;
        r.into_slice(&mut b.wram)?;
        b.wram_bank = (r.u8()? & 7).max(1) as usize;
        r.into_slice(&mut b.hram)?;
        b.ie = r.u8()?;
        b.iff = r.u8()?;
        b.sb = r.u8()?;
        b.sc = r.u8()?;
        b.serial_counter = r.u32()? as i32;
        b.double_speed = r.b()?;
        b.key1_armed = r.b()?;
        b.hdma_src = r.u16()?;
        b.hdma_dst = r.u16()?;
        b.hdma_len = r.u8()?;
        b.hdma_active = r.b()?;
        b.frame_cycles = r.u64()?;

        b.timer.div = r.u16()?;
        b.timer.tima = r.u8()?;
        b.timer.tma = r.u8()?;
        b.timer.tac = r.u8()?;
        b.timer.interrupt = false;

        b.joypad.select = r.u8()?;

        let cart = &mut b.cart;
        let ram_len = r.u32()? as usize;
        if ram_len != cart.ram.len() {
            return None;
        }
        r.into_slice(&mut cart.ram)?;
        cart.ram_enabled = r.b()?;
        cart.rom_bank = r.u32()? as usize;
        cart.ram_bank = r.u32()? as usize;
        cart.bank_hi = r.u32()? as usize;
        cart.mbc1_mode = r.b()?;
        cart.rtc = rtc_read(&mut r)?;
        cart.rtc_latched = rtc_read(&mut r)?;
        cart.rtc_latch_state = r.u8()?;
        cart.ram_dirty = true; // battery contents changed; persist on next autosave

        let p = &mut b.ppu;
        r.into_slice(&mut p.vram)?;
        p.vram_bank = (r.u8()? & 1) as usize;
        r.into_slice(&mut p.oam)?;
        p.lcdc = r.u8()?;
        p.stat = r.u8()?;
        p.scy = r.u8()?;
        p.scx = r.u8()?;
        p.ly = r.u8()?;
        p.lyc = r.u8()?;
        p.bgp = r.u8()?;
        p.obp0 = r.u8()?;
        p.obp1 = r.u8()?;
        p.wy = r.u8()?;
        p.wx = r.u8()?;
        p.bcps = r.u8()?;
        p.ocps = r.u8()?;
        p.opri = r.b()?;
        r.into_slice(&mut p.bg_pal)?;
        r.into_slice(&mut p.ob_pal)?;
        p.mode = r.u8()?;
        p.dot = r.u32()?;
        p.window_line = r.u16()?;
        p.stat_line = r.b()?;
        for px in p.framebuffer.iter_mut() {
            *px = r.u32()?;
        }
        p.int_vblank = false;
        p.int_stat = false;
        p.entered_hblank = false;
        p.frame_ready = false;

        Some(())
    }
}

#[cfg(test)]
mod tests {
    use crate::gameboy::Gameboy;

    fn test_rom() -> Vec<u8> {
        let mut rom = vec![0u8; 0x8000];
        rom[0x100] = 0x00; // NOP
        rom[0x101] = 0xC3; // JP 0x0150
        rom[0x102] = 0x50;
        rom[0x103] = 0x01;
        // busy loop at 0x150: INC A; JR -3
        rom[0x150] = 0x3C;
        rom[0x151] = 0x18;
        rom[0x152] = 0xFD;
        rom[0x134..0x140].copy_from_slice(b"STATETEST\0\0\0");
        rom[0x147] = 0x13; // MBC3+RAM+BATTERY
        rom[0x148] = 0x01; // 64 KiB (we only fill 32; reads pad with 0xFF)
        rom[0x149] = 0x03; // 32 KiB RAM
        rom
    }

    #[test]
    fn state_roundtrip() {
        let mut gb = Gameboy::new(test_rom()).unwrap();
        for _ in 0..10 {
            gb.run_frame();
        }
        let snap = gb.save_state();
        let at_snap = (gb.cpu.pc, gb.cpu.a, gb.bus.frame_cycles);

        for _ in 0..7 {
            gb.run_frame();
        }
        assert_ne!(gb.bus.frame_cycles, at_snap.2);

        assert!(gb.load_state(&snap));
        assert_eq!((gb.cpu.pc, gb.cpu.a, gb.bus.frame_cycles), at_snap);
        // A restored machine must serialize identically.
        assert_eq!(gb.save_state(), snap);
    }

    #[test]
    fn state_rejects_other_game_and_truncation() {
        let mut gb = Gameboy::new(test_rom()).unwrap();
        gb.run_frame();
        let snap = gb.save_state();

        let mut other_rom = test_rom();
        other_rom[0x134..0x139].copy_from_slice(b"OTHER");
        let mut other = Gameboy::new(other_rom).unwrap();
        assert!(!other.load_state(&snap));

        let before = gb.save_state();
        assert!(!gb.load_state(&snap[..snap.len() / 2]));
        assert_eq!(gb.save_state(), before); // untouched on failure
    }
}
