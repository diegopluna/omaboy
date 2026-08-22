// Cartridge: ROM header parsing, MBC1/MBC2/MBC3(+RTC)/MBC5 mappers,
// battery-backed RAM and MBC3 real-time clock (Pokémon G/S/C day & night).

use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, PartialEq)]
pub enum Mbc {
    None,
    Mbc1,
    Mbc2,
    Mbc3,
    Mbc5,
}

#[derive(Clone, Copy, Default)]
pub struct Rtc {
    pub seconds: u8,
    pub minutes: u8,
    pub hours: u8,
    pub days: u16, // 9 bits; overflow -> carry
    pub halted: bool,
    pub day_carry: bool,
    /// Unix time when the registers were last brought up to date.
    pub base: u64,
}

impl Rtc {
    fn now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    /// Advance registers by wall-clock time elapsed since `base`.
    fn tick_to_now(&mut self) {
        let now = Self::now();
        if self.halted {
            self.base = now;
            return;
        }
        let elapsed = now.saturating_sub(self.base);
        self.base = now;
        if elapsed == 0 {
            return;
        }
        let mut total = self.seconds as u64
            + self.minutes as u64 * 60
            + self.hours as u64 * 3600
            + self.days as u64 * 86400
            + elapsed;
        self.seconds = (total % 60) as u8;
        total /= 60;
        self.minutes = (total % 60) as u8;
        total /= 60;
        self.hours = (total % 24) as u8;
        total /= 24;
        if total > 0x1FF {
            self.day_carry = true;
        }
        self.days = (total & 0x1FF) as u16;
    }

    /// 44-byte serialisation (register values + base timestamp).
    pub fn serialize(&self) -> [u8; 44] {
        let mut out = [0u8; 44];
        out[0] = self.seconds;
        out[1] = self.minutes;
        out[2] = self.hours;
        out[3] = (self.days & 0xFF) as u8;
        out[4] = (self.days >> 8) as u8 | (self.halted as u8) << 6 | (self.day_carry as u8) << 7;
        out[8..16].copy_from_slice(&self.base.to_le_bytes());
        out
    }

    pub fn deserialize(&mut self, data: &[u8]) {
        if data.len() < 16 {
            return;
        }
        self.seconds = data[0].min(59);
        self.minutes = data[1].min(59);
        self.hours = data[2].min(23);
        self.days = data[3] as u16 | ((data[4] as u16 & 1) << 8);
        self.halted = data[4] & 0x40 != 0;
        self.day_carry = data[4] & 0x80 != 0;
        self.base = u64::from_le_bytes(data[8..16].try_into().unwrap());
        self.tick_to_now();
    }
}

pub struct Cartridge {
    pub rom: Vec<u8>,
    pub ram: Vec<u8>,
    mbc: Mbc,
    pub has_battery: bool,
    pub has_rtc: bool,
    pub title: String,
    pub cgb: bool,

    rom_bank_count: usize,
    ram_bank_count: usize,

    // Mapper state
    pub(crate) ram_enabled: bool,
    pub(crate) rom_bank: usize,  // current switchable bank
    pub(crate) ram_bank: usize,  // selected RAM bank (or RTC register 0x08..0x0C for MBC3)
    pub(crate) bank_hi: usize,   // MBC1 upper 2 bits / MBC5 9th bit
    pub(crate) mbc1_mode: bool,  // MBC1 banking mode

    pub(crate) rtc: Rtc,
    pub(crate) rtc_latched: Rtc,
    pub(crate) rtc_latch_state: u8,

    pub ram_dirty: bool,
}

impl Cartridge {
    pub fn new(rom: Vec<u8>) -> Result<Self, String> {
        if rom.len() < 0x150 {
            return Err("ROM too small".into());
        }
        let cart_type = rom[0x147];
        let (mbc, has_battery, has_rtc) = match cart_type {
            0x00 | 0x08 | 0x09 => (Mbc::None, cart_type == 0x09, false),
            0x01 | 0x02 => (Mbc::Mbc1, false, false),
            0x03 => (Mbc::Mbc1, true, false),
            0x05 => (Mbc::Mbc2, false, false),
            0x06 => (Mbc::Mbc2, true, false),
            0x0F | 0x10 => (Mbc::Mbc3, true, true),
            0x11 | 0x12 => (Mbc::Mbc3, false, false),
            0x13 => (Mbc::Mbc3, true, false),
            0x19 | 0x1A | 0x1C | 0x1D => (Mbc::Mbc5, false, false),
            0x1B | 0x1E => (Mbc::Mbc5, true, false),
            other => return Err(format!("unsupported cartridge type 0x{:02X}", other)),
        };

        let rom_bank_count = match rom[0x148] {
            n @ 0..=8 => 2usize << n,
            _ => rom.len() / 0x4000,
        }
        .max(2);

        let ram_bank_count = if mbc == Mbc::Mbc2 {
            1 // built-in 512x4 bits, held in one 0x200 "bank"
        } else {
            match rom[0x149] {
                2 => 1,
                3 => 4,
                4 => 16,
                5 => 8,
                _ => 0,
            }
        };
        let ram_size = if mbc == Mbc::Mbc2 { 0x200 } else { ram_bank_count * 0x2000 };

        let title: String = rom[0x134..0x144]
            .iter()
            .take_while(|&&b| b != 0)
            .filter(|&&b| (0x20..0x7F).contains(&b))
            .map(|&b| b as char)
            .collect();

        let cgb = rom[0x143] & 0x80 != 0;

        Ok(Cartridge {
            rom,
            ram: vec![0xFF; ram_size],
            mbc,
            has_battery,
            has_rtc,
            title,
            cgb,
            rom_bank_count,
            ram_bank_count,
            ram_enabled: false,
            rom_bank: 1,
            ram_bank: 0,
            bank_hi: 0,
            mbc1_mode: false,
            rtc: Rtc { base: Rtc::now(), ..Default::default() },
            rtc_latched: Rtc::default(),
            rtc_latch_state: 0xFF,
            ram_dirty: false,
        })
    }

    pub fn read_rom(&self, addr: u16) -> u8 {
        let idx = match addr {
            0x0000..=0x3FFF => {
                // MBC1 mode 1: the fixed bank is also affected by the high bits.
                let bank = if self.mbc == Mbc::Mbc1 && self.mbc1_mode {
                    (self.bank_hi << 5) % self.rom_bank_count
                } else {
                    0
                };
                bank * 0x4000 + addr as usize
            }
            _ => {
                let bank = self.effective_rom_bank();
                bank * 0x4000 + (addr as usize - 0x4000)
            }
        };
        self.rom.get(idx).copied().unwrap_or(0xFF)
    }

    fn effective_rom_bank(&self) -> usize {
        let bank = match self.mbc {
            Mbc::Mbc1 => {
                let lo = if self.rom_bank == 0 { 1 } else { self.rom_bank & 0x1F };
                (self.bank_hi << 5) | lo
            }
            Mbc::Mbc5 => (self.bank_hi << 8) | self.rom_bank,
            _ => {
                if self.rom_bank == 0 {
                    1
                } else {
                    self.rom_bank
                }
            }
        };
        bank % self.rom_bank_count
    }

    pub fn write_rom(&mut self, addr: u16, val: u8) {
        match self.mbc {
            Mbc::None => {}
            Mbc::Mbc1 => match addr {
                0x0000..=0x1FFF => self.ram_enabled = val & 0x0F == 0x0A,
                0x2000..=0x3FFF => self.rom_bank = (val & 0x1F) as usize,
                0x4000..=0x5FFF => self.bank_hi = (val & 0x03) as usize,
                _ => self.mbc1_mode = val & 1 != 0,
            },
            Mbc::Mbc2 => {
                if addr <= 0x3FFF {
                    if addr & 0x0100 == 0 {
                        self.ram_enabled = val & 0x0F == 0x0A;
                    } else {
                        self.rom_bank = (val & 0x0F) as usize;
                    }
                }
            }
            Mbc::Mbc3 => match addr {
                0x0000..=0x1FFF => self.ram_enabled = val & 0x0F == 0x0A,
                0x2000..=0x3FFF => self.rom_bank = (val & 0x7F) as usize,
                0x4000..=0x5FFF => self.ram_bank = (val & 0x0F) as usize,
                _ => {
                    // Latch clock on 0x00 -> 0x01 write sequence.
                    if self.rtc_latch_state == 0x00 && val == 0x01 {
                        self.rtc.tick_to_now();
                        self.rtc_latched = self.rtc;
                    }
                    self.rtc_latch_state = val;
                }
            },
            Mbc::Mbc5 => match addr {
                0x0000..=0x1FFF => self.ram_enabled = val & 0x0F == 0x0A,
                0x2000..=0x2FFF => self.rom_bank = val as usize,
                0x3000..=0x3FFF => self.bank_hi = (val & 1) as usize,
                0x4000..=0x5FFF => self.ram_bank = (val & 0x0F) as usize,
                _ => {}
            },
        }
    }

    pub fn read_ram(&mut self, addr: u16) -> u8 {
        if !self.ram_enabled {
            return 0xFF;
        }
        match self.mbc {
            Mbc::Mbc2 => self.ram[(addr as usize) & 0x1FF] | 0xF0,
            Mbc::Mbc3 if self.ram_bank >= 0x08 => {
                let r = &self.rtc_latched;
                match self.ram_bank {
                    0x08 => r.seconds,
                    0x09 => r.minutes,
                    0x0A => r.hours,
                    0x0B => (r.days & 0xFF) as u8,
                    0x0C => {
                        (r.days >> 8) as u8 & 1
                            | (r.halted as u8) << 6
                            | (r.day_carry as u8) << 7
                    }
                    _ => 0xFF,
                }
            }
            _ => {
                if self.ram_bank_count == 0 {
                    return 0xFF;
                }
                let bank = self.effective_ram_bank();
                self.ram[bank * 0x2000 + addr as usize]
            }
        }
    }

    pub fn write_ram(&mut self, addr: u16, val: u8) {
        if !self.ram_enabled {
            return;
        }
        match self.mbc {
            Mbc::Mbc2 => {
                self.ram[(addr as usize) & 0x1FF] = val | 0xF0;
                self.ram_dirty = true;
            }
            Mbc::Mbc3 if self.ram_bank >= 0x08 => {
                self.rtc.tick_to_now();
                match self.ram_bank {
                    0x08 => self.rtc.seconds = val & 0x3F,
                    0x09 => self.rtc.minutes = val & 0x3F,
                    0x0A => self.rtc.hours = val & 0x1F,
                    0x0B => self.rtc.days = (self.rtc.days & 0x100) | val as u16,
                    0x0C => {
                        self.rtc.days = (self.rtc.days & 0xFF) | ((val as u16 & 1) << 8);
                        self.rtc.halted = val & 0x40 != 0;
                        self.rtc.day_carry = val & 0x80 != 0;
                    }
                    _ => {}
                }
                self.ram_dirty = true;
            }
            _ => {
                if self.ram_bank_count == 0 {
                    return;
                }
                let bank = self.effective_ram_bank();
                self.ram[bank * 0x2000 + addr as usize] = val;
                self.ram_dirty = true;
            }
        }
    }

    fn effective_ram_bank(&self) -> usize {
        let bank = match self.mbc {
            Mbc::Mbc1 => {
                if self.mbc1_mode {
                    self.bank_hi
                } else {
                    0
                }
            }
            _ => self.ram_bank,
        };
        bank % self.ram_bank_count.max(1)
    }

    pub fn load_battery(&mut self, data: &[u8]) {
        let n = data.len().min(self.ram.len());
        self.ram[..n].copy_from_slice(&data[..n]);
        self.ram_dirty = false;
    }

    pub fn rtc_serialize(&mut self) -> [u8; 44] {
        self.rtc.tick_to_now();
        self.rtc.serialize()
    }

    pub fn rtc_deserialize(&mut self, data: &[u8]) {
        self.rtc.deserialize(data);
    }
}

/// Extract a Game Boy ROM from raw bytes: plain .gb/.gbc data passes through,
/// zip archives are searched for the first .gb/.gbc entry.
pub fn extract_rom(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() >= 4 && &data[0..4] == b"PK\x03\x04" {
        extract_from_zip(data)
    } else {
        Ok(data.to_vec())
    }
}

fn extract_from_zip(data: &[u8]) -> Result<Vec<u8>, String> {
    // Locate End Of Central Directory record (scan backwards for signature).
    let eocd = data
        .windows(4)
        .rposition(|w| w == b"PK\x05\x06")
        .ok_or("zip: no end-of-central-directory record")?;
    let rd = |off: usize, n: usize| -> u64 {
        let mut v = 0u64;
        for i in 0..n {
            v |= (*data.get(off + i).unwrap_or(&0) as u64) << (8 * i);
        }
        v
    };
    let entries = rd(eocd + 10, 2) as usize;
    let mut off = rd(eocd + 16, 4) as usize;

    for _ in 0..entries {
        if data.len() < off + 46 || &data[off..off + 4] != b"PK\x01\x02" {
            break;
        }
        let method = rd(off + 10, 2);
        let comp_size = rd(off + 20, 4) as usize;
        let name_len = rd(off + 28, 2) as usize;
        let extra_len = rd(off + 30, 2) as usize;
        let comment_len = rd(off + 32, 2) as usize;
        let local_off = rd(off + 42, 4) as usize;
        let name = String::from_utf8_lossy(&data[off + 46..off + 46 + name_len]).to_lowercase();

        if name.ends_with(".gb") || name.ends_with(".gbc") {
            // Local header: skip its (possibly different) name/extra lengths.
            if data.len() < local_off + 30 || &data[local_off..local_off + 4] != b"PK\x03\x04" {
                return Err("zip: bad local header".into());
            }
            let lname = rd(local_off + 26, 2) as usize;
            let lextra = rd(local_off + 28, 2) as usize;
            let start = local_off + 30 + lname + lextra;
            let comp = data
                .get(start..start + comp_size)
                .ok_or("zip: truncated data")?;
            return match method {
                0 => Ok(comp.to_vec()),
                8 => {
                    use std::io::Read;
                    let mut out = Vec::new();
                    flate2::read::DeflateDecoder::new(comp)
                        .read_to_end(&mut out)
                        .map_err(|e| format!("zip: inflate failed: {e}"))?;
                    Ok(out)
                }
                m => Err(format!("zip: unsupported compression method {m}")),
            };
        }
        off += 46 + name_len + extra_len + comment_len;
    }
    Err("zip: no .gb/.gbc file found inside".into())
}
