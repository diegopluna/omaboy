// Pixel Processing Unit: scanline renderer, DMG + CGB modes.
//
// Output is 160x144 0xFFRRGGBB (QImage::Format_RGB32 compatible).
// In DMG mode the four shades map through a configurable palette so the
// screen can match the active Omarchy theme.

pub const WIDTH: usize = 160;
pub const HEIGHT: usize = 144;

const MODE_HBLANK: u8 = 0;
const MODE_VBLANK: u8 = 1;
const MODE_OAM: u8 = 2;
const MODE_DRAW: u8 = 3;

pub struct Ppu {
    pub cgb: bool,
    pub vram: [u8; 0x4000], // two banks of 0x2000
    pub vram_bank: usize,
    pub oam: [u8; 0xA0],

    pub lcdc: u8,
    pub stat: u8,
    pub scy: u8,
    pub scx: u8,
    pub ly: u8,
    pub lyc: u8,
    pub bgp: u8,
    pub obp0: u8,
    pub obp1: u8,
    pub wy: u8,
    pub wx: u8,
    pub opri: bool, // OAM-index priority (CGB default) vs X priority (DMG)

    // CGB palette RAM
    pub bcps: u8,
    pub ocps: u8,
    pub bg_pal: [u8; 64],
    pub ob_pal: [u8; 64],

    pub(crate) mode: u8,
    pub(crate) dot: u32,
    pub(crate) window_line: u16,
    pub(crate) stat_line: bool,

    /// Palette for DMG shades 0..3 (0 = lightest on hardware).
    pub dmg_colors: [u32; 4],

    pub framebuffer: [u32; WIDTH * HEIGHT],
    pub frame_ready: bool,
    pub entered_hblank: bool, // consumed by the bus for HBlank DMA

    pub int_vblank: bool,
    pub int_stat: bool,

    /// Raw RGB555 vs color-corrected CGB output.
    pub color_correction: bool,

    // Per-line scratch: BG color index (0-3) and BG-priority flag per pixel.
    line_bg_index: [u8; WIDTH],
    line_bg_prio: [bool; WIDTH],
}

impl Ppu {
    pub fn new(cgb: bool) -> Self {
        Ppu {
            cgb,
            vram: [0; 0x4000],
            vram_bank: 0,
            oam: [0; 0xA0],
            lcdc: 0x91,
            stat: 0x85,
            scy: 0,
            scx: 0,
            ly: 0,
            lyc: 0,
            bgp: 0xFC,
            obp0: 0xFF,
            obp1: 0xFF,
            wy: 0,
            wx: 0,
            opri: cgb,
            bcps: 0,
            ocps: 0,
            bg_pal: [0xFF; 64],
            ob_pal: [0xFF; 64],
            mode: MODE_OAM,
            dot: 0,
            window_line: 0,
            stat_line: false,
            dmg_colors: [0xFFE0F8D0, 0xFF88C070, 0xFF346856, 0xFF081820],
            framebuffer: [0xFF000000; WIDTH * HEIGHT],
            frame_ready: false,
            entered_hblank: false,
            int_vblank: false,
            int_stat: false,
            color_correction: true,
            line_bg_index: [0; WIDTH],
            line_bg_prio: [false; WIDTH],
        }
    }

    #[inline]
    fn lcd_on(&self) -> bool {
        self.lcdc & 0x80 != 0
    }

    pub fn read_vram(&self, addr: u16) -> u8 {
        self.vram[self.vram_bank * 0x2000 + (addr as usize & 0x1FFF)]
    }

    pub fn write_vram(&mut self, addr: u16, val: u8) {
        self.vram[self.vram_bank * 0x2000 + (addr as usize & 0x1FFF)] = val;
    }

    pub fn read_reg(&self, addr: u16) -> u8 {
        match addr {
            0xFF40 => self.lcdc,
            0xFF41 => {
                let coincidence = (self.ly == self.lyc) as u8;
                0x80 | (self.stat & 0x78) | (coincidence << 2) | if self.lcd_on() { self.mode } else { 0 }
            }
            0xFF42 => self.scy,
            0xFF43 => self.scx,
            0xFF44 => self.ly,
            0xFF45 => self.lyc,
            0xFF47 => self.bgp,
            0xFF48 => self.obp0,
            0xFF49 => self.obp1,
            0xFF4A => self.wy,
            0xFF4B => self.wx,
            0xFF4F => 0xFE | self.vram_bank as u8,
            0xFF68 => self.bcps | 0x40,
            0xFF69 => self.bg_pal[(self.bcps & 0x3F) as usize],
            0xFF6A => self.ocps | 0x40,
            0xFF6B => self.ob_pal[(self.ocps & 0x3F) as usize],
            0xFF6C => 0xFE | !self.opri as u8,
            _ => 0xFF,
        }
    }

    pub fn write_reg(&mut self, addr: u16, val: u8) {
        match addr {
            0xFF40 => {
                let was_on = self.lcd_on();
                self.lcdc = val;
                if was_on && !self.lcd_on() {
                    self.ly = 0;
                    self.dot = 0;
                    self.mode = MODE_HBLANK;
                    self.window_line = 0;
                } else if !was_on && self.lcd_on() {
                    self.dot = 0;
                    self.mode = MODE_OAM;
                    self.update_stat_irq();
                }
            }
            0xFF41 => {
                self.stat = (self.stat & 0x07) | (val & 0x78);
                self.update_stat_irq();
            }
            0xFF42 => self.scy = val,
            0xFF43 => self.scx = val,
            0xFF45 => {
                self.lyc = val;
                self.update_stat_irq();
            }
            0xFF47 => self.bgp = val,
            0xFF48 => self.obp0 = val,
            0xFF49 => self.obp1 = val,
            0xFF4A => self.wy = val,
            0xFF4B => self.wx = val,
            0xFF4F => {
                if self.cgb {
                    self.vram_bank = (val & 1) as usize;
                }
            }
            0xFF68 => self.bcps = val & 0xBF,
            0xFF69 => {
                self.bg_pal[(self.bcps & 0x3F) as usize] = val;
                if self.bcps & 0x80 != 0 {
                    self.bcps = 0x80 | (self.bcps.wrapping_add(1) & 0x3F);
                }
            }
            0xFF6A => self.ocps = val & 0xBF,
            0xFF6B => {
                self.ob_pal[(self.ocps & 0x3F) as usize] = val;
                if self.ocps & 0x80 != 0 {
                    self.ocps = 0x80 | (self.ocps.wrapping_add(1) & 0x3F);
                }
            }
            0xFF6C => self.opri = val & 1 == 0,
            _ => {}
        }
    }

    /// Advance the PPU by `dots` (4.19 MHz cycles).
    pub fn tick(&mut self, dots: u32) {
        if !self.lcd_on() {
            return;
        }
        for _ in 0..dots {
            self.dot += 1;
            match self.mode {
                MODE_OAM => {
                    if self.dot == 80 {
                        self.mode = MODE_DRAW;
                    }
                }
                MODE_DRAW => {
                    if self.dot == 80 + 172 {
                        self.render_scanline();
                        self.mode = MODE_HBLANK;
                        self.entered_hblank = true;
                        self.update_stat_irq();
                    }
                }
                MODE_HBLANK => {
                    if self.dot == 456 {
                        self.dot = 0;
                        self.ly += 1;
                        if self.ly == 144 {
                            self.mode = MODE_VBLANK;
                            self.int_vblank = true;
                            self.frame_ready = true;
                        } else {
                            self.mode = MODE_OAM;
                        }
                        self.update_stat_irq();
                    }
                }
                _ => {
                    // VBLANK
                    if self.dot == 456 {
                        self.dot = 0;
                        self.ly += 1;
                        if self.ly > 153 {
                            self.ly = 0;
                            self.window_line = 0;
                            self.mode = MODE_OAM;
                        }
                        self.update_stat_irq();
                    }
                }
            }
        }
    }

    /// STAT interrupt line: rising-edge triggered ("STAT blocking").
    fn update_stat_irq(&mut self) {
        if !self.lcd_on() {
            self.stat_line = false;
            return;
        }
        let mut line = false;
        if self.stat & 0x40 != 0 && self.ly == self.lyc {
            line = true;
        }
        match self.mode {
            MODE_HBLANK if self.stat & 0x08 != 0 => line = true,
            MODE_VBLANK if self.stat & 0x10 != 0 => line = true,
            // OAM interrupt also fires on VBlank entry line
            MODE_OAM if self.stat & 0x20 != 0 => line = true,
            MODE_VBLANK if self.stat & 0x20 != 0 && self.ly == 144 => line = true,
            _ => {}
        }
        if line && !self.stat_line {
            self.int_stat = true;
        }
        self.stat_line = line;
    }

    // ---- rendering ----

    fn render_scanline(&mut self) {
        let y = self.ly as usize;
        if y >= HEIGHT {
            return;
        }
        self.line_bg_index = [0; WIDTH];
        self.line_bg_prio = [false; WIDTH];

        // LCDC bit 0: DMG = BG+window off (white); CGB = sprites lose priority.
        let bg_enabled = self.cgb || self.lcdc & 0x01 != 0;
        if bg_enabled {
            self.render_bg_line(y);
        } else {
            let c = self.dmg_colors[0];
            for x in 0..WIDTH {
                self.framebuffer[y * WIDTH + x] = c;
            }
        }
        if self.lcdc & 0x02 != 0 {
            self.render_sprites_line(y);
        }
    }

    fn render_bg_line(&mut self, y: usize) {
        let window_active =
            self.lcdc & 0x20 != 0 && self.wy as usize <= y && self.wx < 167;
        let mut window_drawn = false;

        for x in 0..WIDTH {
            let in_window = window_active && x + 7 >= self.wx as usize;
            let (map_base, tx, ty) = if in_window {
                window_drawn = true;
                let wx = x + 7 - self.wx as usize;
                let wy = self.window_line as usize;
                let base = if self.lcdc & 0x40 != 0 { 0x1C00 } else { 0x1800 };
                (base, wx, wy)
            } else {
                let bx = (x + self.scx as usize) & 0xFF;
                let by = (y + self.scy as usize) & 0xFF;
                let base = if self.lcdc & 0x08 != 0 { 0x1C00 } else { 0x1800 };
                (base, bx, by)
            };

            let map_idx = map_base + (ty / 8) * 32 + tx / 8;
            let tile_num = self.vram[map_idx];
            let attr = if self.cgb { self.vram[0x2000 + map_idx] } else { 0 };

            let tile_bank = ((attr >> 3) & 1) as usize;
            let palette = (attr & 0x07) as usize;
            let x_flip = attr & 0x20 != 0;
            let y_flip = attr & 0x40 != 0;
            let priority = attr & 0x80 != 0;

            let mut py = ty % 8;
            if y_flip {
                py = 7 - py;
            }
            let mut px = tx % 8;
            if x_flip {
                px = 7 - px;
            }

            let tile_addr = if self.lcdc & 0x10 != 0 {
                tile_num as usize * 16
            } else {
                (0x1000i32 + (tile_num as i8 as i32) * 16) as usize
            };
            let row = tile_bank * 0x2000 + tile_addr + py * 2;
            let lo = (self.vram[row] >> (7 - px)) & 1;
            let hi = (self.vram[row + 1] >> (7 - px)) & 1;
            let color_idx = (hi << 1) | lo;

            self.line_bg_index[x] = color_idx;
            self.line_bg_prio[x] = priority;

            let color = if self.cgb {
                self.cgb_color(&self.bg_pal, palette, color_idx)
            } else {
                self.dmg_colors[((self.bgp >> (color_idx * 2)) & 3) as usize]
            };
            self.framebuffer[y * WIDTH + x] = color;
        }

        if window_drawn {
            self.window_line += 1;
        }
    }

    fn render_sprites_line(&mut self, y: usize) {
        let tall = self.lcdc & 0x04 != 0;
        let height = if tall { 16 } else { 8 };

        // Collect first 10 visible sprites in OAM order.
        let mut visible: Vec<(usize, i32)> = Vec::with_capacity(10); // (oam index, x)
        for i in 0..40 {
            let sy = self.oam[i * 4] as i32 - 16;
            if (y as i32) >= sy && (y as i32) < sy + height {
                visible.push((i, self.oam[i * 4 + 1] as i32 - 8));
                if visible.len() == 10 {
                    break;
                }
            }
        }
        // DMG: smaller X wins; ties broken by OAM index. CGB: OAM index wins.
        if !self.opri {
            visible.sort_by_key(|&(i, x)| (x, i));
        }

        let mut sprite_drawn = [false; WIDTH];
        for &(i, sx) in &visible {
            let sy = self.oam[i * 4] as i32 - 16;
            let mut tile = self.oam[i * 4 + 2];
            let attr = self.oam[i * 4 + 3];
            if tall {
                tile &= 0xFE;
            }
            let x_flip = attr & 0x20 != 0;
            let y_flip = attr & 0x40 != 0;
            let behind_bg = attr & 0x80 != 0;
            let bank = if self.cgb { ((attr >> 3) & 1) as usize } else { 0 };

            let mut row = (y as i32 - sy) as usize;
            if y_flip {
                row = height as usize - 1 - row;
            }
            let addr = bank * 0x2000 + tile as usize * 16 + row * 2;
            let lo_byte = self.vram[addr];
            let hi_byte = self.vram[addr + 1];

            for px in 0..8i32 {
                let x = sx + px;
                if !(0..WIDTH as i32).contains(&x) {
                    continue;
                }
                let x = x as usize;
                if sprite_drawn[x] {
                    continue; // a higher-priority sprite already owns this pixel
                }
                let bit = if x_flip { px } else { 7 - px };
                let lo = (lo_byte >> bit) & 1;
                let hi = (hi_byte >> bit) & 1;
                let color_idx = (hi << 1) | lo;
                if color_idx == 0 {
                    continue;
                }

                // BG-vs-OBJ priority. CGB LCDC bit0 = master priority: when
                // clear, sprites always win.
                let bg_idx = self.line_bg_index[x];
                let master = !self.cgb || self.lcdc & 0x01 != 0;
                if master && bg_idx != 0 && (behind_bg || (self.cgb && self.line_bg_prio[x])) {
                    sprite_drawn[x] = true;
                    continue;
                }

                let color = if self.cgb {
                    self.cgb_color(&self.ob_pal, (attr & 0x07) as usize, color_idx)
                } else {
                    let pal = if attr & 0x10 != 0 { self.obp1 } else { self.obp0 };
                    self.dmg_colors[((pal >> (color_idx * 2)) & 3) as usize]
                };
                self.framebuffer[y * WIDTH + x] = color;
                sprite_drawn[x] = true;
            }
        }
    }

    fn cgb_color(&self, pal_ram: &[u8; 64], palette: usize, color_idx: u8) -> u32 {
        let off = palette * 8 + color_idx as usize * 2;
        let raw = pal_ram[off] as u16 | ((pal_ram[off + 1] as u16) << 8);
        let r = (raw & 0x1F) as u32;
        let g = ((raw >> 5) & 0x1F) as u32;
        let b = ((raw >> 10) & 0x1F) as u32;
        if self.color_correction {
            // Blend channels to approximate the CGB LCD's muted response.
            let rr = (r * 26 + g * 4 + b * 2).min(960) / 4;
            let gg = (g * 24 + b * 8).min(960) / 4;
            let bb = (r * 6 + g * 4 + b * 22).min(960) / 4;
            0xFF000000 | (rr.min(255) << 16) | (gg.min(255) << 8) | bb.min(255)
        } else {
            let e = |c: u32| (c << 3) | (c >> 2);
            0xFF000000 | (e(r) << 16) | (e(g) << 8) | e(b)
        }
    }
}
