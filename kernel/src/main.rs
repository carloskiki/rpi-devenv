#![no_std]
#![no_main]

use core::arch::asm;
use core::iter;
use core::sync::atomic::{AtomicBool, Ordering};
use core::{arch::global_asm, hint::black_box};

use rpi::gpio::{Alternate5, Pin};
use rpi::mmu::{AccessPermissions, MemoryAttributes, MemoryType, SectionBaseAddress, SectionDescriptor, TRANSLATION_TABLE};
use rpi::uart::MiniUart;
use rpi::{data_memory_barrier, data_synchronization_barrier};

// const GPFSEL3: usize = 0x2020000C;
const GPFSEL4: usize = 0x20200010;
const GPSET1: usize = 0x20200020;
const GPCLR1: usize = 0x2020002C;

const TIMER_FOUR_SEC: u32 = 0x400000;

const BOOT_ADDRESS: usize = 0x8000;

global_asm!(include_str!("boot.s"), options(raw));

#[no_mangle]
pub extern "C" fn first_stage() -> ! {
    let mut uart = unsafe { MiniUart::get_unchecked() };
    uart.set_bit_mode(true);
    uart.set_baud_rate(115200);
    let tx_pin: Pin<14, Alternate5> = Pin::get().unwrap();
    let rx_pin: Pin<15, Alternate5> = Pin::get().unwrap();
    let mut rx_tx = uart.enable_transmitter(tx_pin).enable_receiver(rx_pin);
    rx_tx.send_blocking("hello world\n".bytes());

    let _test = SectionDescriptor::new(
            SectionBaseAddress::SuperSection(1 << 5),
            AccessPermissions::ReadWrite,
            MemoryAttributes {
                execute: false,
                global: true,
                memory_type: MemoryType::Device { shareable: false },
            },
    );

    unsafe {
        let addr = (TRANSLATION_TABLE.0.as_ptr() as usize) & (!0 << 14) | 0b11011;

        // Set the translation table base address with its flags
        asm!("mcr p15, 0, {}, c2, c0, 0", in(reg) addr, options(nostack, nomem, preserves_flags));
        // Set the value of `N` to 0 in the translation table base control register
        asm!("mcr p15, 0, {}, c2, c0, 2", in(reg) 0, options(nostack, nomem, preserves_flags));
        // Set all domains to manager mode
        asm!("mcr p15, 0, {}, c3, c0, 0", in(reg) !0, options(nostack, nomem, preserves_flags));

        // Invalidate caches and TLB
        asm!("mcr p15, 0, {0}, c7, c7, 0
              mcr p15, 0, {0}, c8, c7, 0", in(reg) 0, options(nostack, nomem, preserves_flags));
        // Instruction cache is disabled by default, and thus does not need invalidation.
        //
        // bit 12: enable L1 instruction cache
        // bit 11: enable branch prediction
        // bit 2: enable L1 data cache
        // bit 0: enable MMU
        let mut control_reg: u32;
        asm!("mrc p15, 0, {}, c1, c0, 0", out(reg) control_reg, options(nostack, nomem, preserves_flags));
        control_reg |= 0b101;
        control_reg |= 0b11 << 11;
        asm!("mcr p15, 0, {}, c1, c0, 0", in(reg) control_reg, options(nostack, nomem, preserves_flags));
    }
    data_synchronization_barrier();

    rx_tx.send_blocking("MMU enabled\n".bytes());
    let atomic_test = AtomicBool::new(false);
    atomic_test.fetch_or(true, Ordering::Acquire);
    rx_tx.send_blocking("Atomic test passed\n".bytes());

    loop {}
}

#[repr(u8)]
enum ProcessorMode {
    User = 0b10000,
    Fiq = 0b10001,
    Irq = 0b10010,
    Supervisor = 0b10011,
    Abort = 0b10111,
    Undefined = 0b11011,
    System = 0b11111,
}

unsafe fn get32(addr: usize) -> u32 {
    core::ptr::read_volatile(addr as *const u32)
}

unsafe fn put32(adder: usize, value: u32) {
    core::ptr::write_volatile(adder as *mut u32, value);
}

fn delay(mut n: u32) {
    while n > 0 {
        n -= 1;
        black_box(n);
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    data_memory_barrier();

    // Set GPIO pin 47 as output
    unsafe {
        get32(GPFSEL4);
        let mask = 0b111 << 21;
        let output = 0b001 << 21;
        put32(GPFSEL4, (get32(GPFSEL4) & !mask) | output);
    }

    loop {
        // Turn off the LED
        unsafe {
            put32(GPSET1, 1 << 15);
        }
        delay(TIMER_FOUR_SEC);

        // Turn on the LED
        unsafe {
            put32(GPCLR1, 1 << 15);
        }
        delay(TIMER_FOUR_SEC);
    }
}
