// Headless test harness: run a ROM for N frames, print serial output
// (blargg test ROMs report results there), optionally dump the frame as PPM.
//
// Usage: headless <rom> [frames] [out.ppm]

use omaboy_core::Gameboy;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: headless <rom> [frames] [out.ppm]");
        std::process::exit(2);
    }
    let rom = std::fs::read(&args[1]).expect("read rom");
    let frames: u32 = args.get(2).map(|s| s.parse().unwrap()).unwrap_or(600);
    let mut gb = Gameboy::new(rom).expect("load rom");

    let mut serial = String::new();
    for _ in 0..frames {
        gb.run_frame();
        gb.bus.apu.samples.clear();
        for b in gb.bus.serial_out.drain(..) {
            serial.push(b as char);
        }
    }
    if !serial.is_empty() {
        println!("SERIAL: {}", serial);
    }

    if let Some(path) = args.get(3) {
        let fb = &gb.bus.ppu.framebuffer;
        let mut out = format!("P6\n160 144\n255\n").into_bytes();
        for px in fb.iter() {
            out.push((px >> 16) as u8);
            out.push((px >> 8) as u8);
            out.push(*px as u8);
        }
        std::fs::write(path, out).expect("write ppm");
        println!("wrote {}", path);
    }
}
