// C ABI of the Rust emulator core (core/src/lib.rs).
#pragma once

#include <cstddef>
#include <cstdint>

extern "C" {

struct GbHandle;

GbHandle *gb_create(const uint8_t *data, size_t len);
void gb_destroy(GbHandle *h);
void gb_reset(GbHandle *h);
void gb_run_frame(GbHandle *h);

// Bitmask: 0=Right 1=Left 2=Up 3=Down 4=A 5=B 6=Select 7=Start
void gb_set_buttons(GbHandle *h, uint8_t buttons);

const uint32_t *gb_framebuffer(const GbHandle *h); // 160x144, 0xFFRRGGBB

size_t gb_audio_read(GbHandle *h, float *out, size_t max);
size_t gb_audio_pending(const GbHandle *h);
void gb_audio_clear(GbHandle *h);

bool gb_is_cgb(const GbHandle *h);
bool gb_has_battery(const GbHandle *h);
bool gb_has_rtc(const GbHandle *h);
void gb_title(const GbHandle *h, char *out, size_t cap);

size_t gb_battery_size(const GbHandle *h);
const uint8_t *gb_battery_data(const GbHandle *h);
void gb_battery_load(GbHandle *h, const uint8_t *data, size_t len);
bool gb_battery_take_dirty(GbHandle *h);

size_t gb_rtc_save(GbHandle *h, uint8_t *out, size_t cap);
void gb_rtc_load(GbHandle *h, const uint8_t *data, size_t len);

size_t gb_state_save(GbHandle *h, uint8_t *out, size_t cap);
bool gb_state_load(GbHandle *h, const uint8_t *data, size_t len);

void gb_set_dmg_palette(GbHandle *h, const uint32_t *colors);
void gb_set_color_correction(GbHandle *h, bool on);

size_t gb_serial_take(GbHandle *h, uint8_t *out, size_t cap);

} // extern "C"
