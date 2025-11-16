//! Angzarr Kernel - Hello World Bootable Kernel
//!
//! This is a minimal bootable kernel that demonstrates the Angzarr infrastructure.

#![no_std]
#![no_main]
#![feature(lang_items)]

use core::panic::PanicInfo;

/// VGA text buffer address
const VGA_BUFFER: *mut u8 = 0xb8000 as *mut u8;

/// VGA color codes
#[allow(dead_code)]
#[repr(u8)]
enum Color {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}

/// Make a color byte from foreground and background colors
const fn color_byte(fg: Color, bg: Color) -> u8 {
    (bg as u8) << 4 | (fg as u8)
}

/// Write a string to VGA text buffer
unsafe fn vga_print(s: &str, color: u8) {
    let mut offset = 0;
    for byte in s.bytes() {
        *VGA_BUFFER.offset(offset) = byte;
        *VGA_BUFFER.offset(offset + 1) = color;
        offset += 2;
    }
}

/// Kernel entry point
#[no_mangle]
pub extern "C" fn _start() -> ! {
    unsafe {
        // Clear screen (80x25 characters)
        for i in 0..(80 * 25 * 2) {
            *VGA_BUFFER.offset(i) = 0;
        }

        // Print "Hello World from Angzarr!" in green on black
        let color = color_byte(Color::LightGreen, Color::Black);
        vga_print("Hello World from Angzarr!", color);

        // Print kernel info on second line
        let info_color = color_byte(Color::LightCyan, Color::Black);
        let info = "Rust Kernel - Phase 1 Complete";
        let info_offset = 80 * 2; // Second line
        let mut offset = 0;
        for byte in info.bytes() {
            *VGA_BUFFER.offset(info_offset + offset) = byte;
            *VGA_BUFFER.offset(info_offset + offset + 1) = info_color;
            offset += 2;
        }

        // Print success message on third line
        let success_color = color_byte(Color::Yellow, Color::Black);
        let success = "Core data structures: READY";
        let success_offset = 160 * 2; // Third line
        let mut offset = 0;
        for byte in success.bytes() {
            *VGA_BUFFER.offset(success_offset + offset) = byte;
            *VGA_BUFFER.offset(success_offset + offset + 1) = success_color;
            offset += 2;
        }
    }

    // Halt the CPU
    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}

/// Panic handler - required for no_std
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    unsafe {
        let color = color_byte(Color::White, Color::Red);
        vga_print("KERNEL PANIC!", color);

        if let Some(location) = info.location() {
            // This is simplified - in real kernel we'd format properly
            vga_print(" at ", color);
        }
    }

    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}

/// Language item for eh_personality (required for unwinding, but we panic=abort)
#[lang = "eh_personality"]
extern "C" fn eh_personality() {}
