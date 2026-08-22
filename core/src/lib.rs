// omaboy-core: Game Boy / Game Boy Color emulator core with a C ABI.

pub mod apu;
pub mod bus;
pub mod cartridge;
pub mod cpu;
pub mod gameboy;
pub mod joypad;
pub mod ppu;
pub mod state;
pub mod timer;

pub use gameboy::Gameboy;

use std::ffi::c_char;
use std::slice;

pub struct GbHandle {
    gb: Gameboy,
}

/// Create an emulator from ROM bytes (plain .gb/.gbc or a zip containing one).
/// Returns null on failure.
#[no_mangle]
pub extern "C" fn gb_create(data: *const u8, len: usize) -> *mut GbHandle {
    if data.is_null() || len == 0 {
        return std::ptr::null_mut();
    }
    let bytes = unsafe { slice::from_raw_parts(data, len) };
    let rom = match cartridge::extract_rom(bytes) {
        Ok(r) => r,
        Err(_) => return std::ptr::null_mut(),
    };
    match Gameboy::new(rom) {
        Ok(gb) => Box::into_raw(Box::new(GbHandle { gb })),
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn gb_destroy(h: *mut GbHandle) {
    if !h.is_null() {
        drop(unsafe { Box::from_raw(h) });
    }
}

#[no_mangle]
pub extern "C" fn gb_reset(h: *mut GbHandle) {
    let h = unsafe { &mut *h };
    h.gb.reset();
}

/// Run one video frame (~16.74 ms of emulated time).
#[no_mangle]
pub extern "C" fn gb_run_frame(h: *mut GbHandle) {
    let h = unsafe { &mut *h };
    h.gb.run_frame();
}

/// Buttons bitmask: 0=Right 1=Left 2=Up 3=Down 4=A 5=B 6=Select 7=Start.
#[no_mangle]
pub extern "C" fn gb_set_buttons(h: *mut GbHandle, buttons: u8) {
    let h = unsafe { &mut *h };
    h.gb.bus.joypad.set_buttons(buttons);
}

/// 160x144 pixels, 0xFFRRGGBB (QImage::Format_RGB32). Valid until destroy.
#[no_mangle]
pub extern "C" fn gb_framebuffer(h: *const GbHandle) -> *const u32 {
    let h = unsafe { &*h };
    h.gb.bus.ppu.framebuffer.as_ptr()
}

/// Drain up to `max` f32 samples (interleaved stereo, 48 kHz) into `out`.
/// Returns the number of samples written.
#[no_mangle]
pub extern "C" fn gb_audio_read(h: *mut GbHandle, out: *mut f32, max: usize) -> usize {
    let h = unsafe { &mut *h };
    let buf = &mut h.gb.bus.apu.samples;
    let n = buf.len().min(max & !1);
    if n > 0 && !out.is_null() {
        unsafe { std::ptr::copy_nonoverlapping(buf.as_ptr(), out, n) };
        buf.drain(..n);
    }
    n
}

#[no_mangle]
pub extern "C" fn gb_audio_pending(h: *const GbHandle) -> usize {
    let h = unsafe { &*h };
    h.gb.bus.apu.samples.len()
}

#[no_mangle]
pub extern "C" fn gb_audio_clear(h: *mut GbHandle) {
    let h = unsafe { &mut *h };
    h.gb.bus.apu.samples.clear();
}

#[no_mangle]
pub extern "C" fn gb_is_cgb(h: *const GbHandle) -> bool {
    let h = unsafe { &*h };
    h.gb.bus.cgb
}

#[no_mangle]
pub extern "C" fn gb_has_battery(h: *const GbHandle) -> bool {
    let h = unsafe { &*h };
    h.gb.bus.cart.has_battery
}

#[no_mangle]
pub extern "C" fn gb_has_rtc(h: *const GbHandle) -> bool {
    let h = unsafe { &*h };
    h.gb.bus.cart.has_rtc
}

/// Copy the ROM title into `out` (nul-terminated). `cap` includes the nul.
#[no_mangle]
pub extern "C" fn gb_title(h: *const GbHandle, out: *mut c_char, cap: usize) {
    let h = unsafe { &*h };
    if out.is_null() || cap == 0 {
        return;
    }
    let title = h.gb.bus.cart.title.as_bytes();
    let n = title.len().min(cap - 1);
    unsafe {
        std::ptr::copy_nonoverlapping(title.as_ptr() as *const c_char, out, n);
        *out.add(n) = 0;
    }
}

/// Battery-backed RAM access (save files).
#[no_mangle]
pub extern "C" fn gb_battery_size(h: *const GbHandle) -> usize {
    let h = unsafe { &*h };
    h.gb.bus.cart.ram.len()
}

#[no_mangle]
pub extern "C" fn gb_battery_data(h: *const GbHandle) -> *const u8 {
    let h = unsafe { &*h };
    h.gb.bus.cart.ram.as_ptr()
}

#[no_mangle]
pub extern "C" fn gb_battery_load(h: *mut GbHandle, data: *const u8, len: usize) {
    let h = unsafe { &mut *h };
    if !data.is_null() && len > 0 {
        let d = unsafe { slice::from_raw_parts(data, len) };
        h.gb.bus.cart.load_battery(d);
    }
}

/// True if battery RAM changed since the last save; clears the flag.
#[no_mangle]
pub extern "C" fn gb_battery_take_dirty(h: *mut GbHandle) -> bool {
    let h = unsafe { &mut *h };
    std::mem::take(&mut h.gb.bus.cart.ram_dirty)
}

/// RTC state, 44 bytes. Returns bytes written (0 if no RTC).
#[no_mangle]
pub extern "C" fn gb_rtc_save(h: *mut GbHandle, out: *mut u8, cap: usize) -> usize {
    let h = unsafe { &mut *h };
    if !h.gb.bus.cart.has_rtc || out.is_null() || cap < 44 {
        return 0;
    }
    let data = h.gb.bus.cart.rtc_serialize();
    unsafe { std::ptr::copy_nonoverlapping(data.as_ptr(), out, 44) };
    44
}

#[no_mangle]
pub extern "C" fn gb_rtc_load(h: *mut GbHandle, data: *const u8, len: usize) {
    let h = unsafe { &mut *h };
    if !data.is_null() && len > 0 {
        let d = unsafe { slice::from_raw_parts(data, len) };
        h.gb.bus.cart.rtc_deserialize(d);
    }
}

/// Set the 4 DMG shade colors (0xAARRGGBB, index 0 = lightest shade).
#[no_mangle]
pub extern "C" fn gb_set_dmg_palette(h: *mut GbHandle, colors: *const u32) {
    let h = unsafe { &mut *h };
    if colors.is_null() {
        return;
    }
    let c = unsafe { slice::from_raw_parts(colors, 4) };
    h.gb.bus.ppu.dmg_colors.copy_from_slice(c);
}

#[no_mangle]
pub extern "C" fn gb_set_color_correction(h: *mut GbHandle, on: bool) {
    let h = unsafe { &mut *h };
    h.gb.bus.ppu.color_correction = on;
}

/// Serialize the machine state. With `out` null, returns the required size;
/// otherwise writes up to `cap` bytes and returns bytes written (0 = too small).
#[no_mangle]
pub extern "C" fn gb_state_save(h: *mut GbHandle, out: *mut u8, cap: usize) -> usize {
    let h = unsafe { &mut *h };
    let data = h.gb.save_state();
    if out.is_null() {
        return data.len();
    }
    if cap < data.len() {
        return 0;
    }
    unsafe { std::ptr::copy_nonoverlapping(data.as_ptr(), out, data.len()) };
    data.len()
}

/// Restore a snapshot. Fails (false) on corrupt data or a different game;
/// the running state is untouched on failure.
#[no_mangle]
pub extern "C" fn gb_state_load(h: *mut GbHandle, data: *const u8, len: usize) -> bool {
    let h = unsafe { &mut *h };
    if data.is_null() || len == 0 {
        return false;
    }
    let d = unsafe { slice::from_raw_parts(data, len) };
    h.gb.load_state(d)
}

/// Drain serial output (used by test ROMs). Returns bytes written.
#[no_mangle]
pub extern "C" fn gb_serial_take(h: *mut GbHandle, out: *mut u8, cap: usize) -> usize {
    let h = unsafe { &mut *h };
    let buf = &mut h.gb.bus.serial_out;
    let n = buf.len().min(cap);
    if n > 0 && !out.is_null() {
        unsafe { std::ptr::copy_nonoverlapping(buf.as_ptr(), out, n) };
    }
    buf.drain(..n);
    n
}
