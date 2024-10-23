#![no_std]
#![no_main]

use core::arch::asm;
use core::{arch::global_asm, hint::black_box};

use rpi::mmu::TRANSLATION_TABLE;
use rpi::{data_memory_barrier, data_synchronization_barrier};

// const GPFSEL3: usize = 0x2020000C;
const GPFSEL4: usize = 0x20200010;
const GPSET1: usize = 0x20200020;
const GPCLR1: usize = 0x2020002C;

const BOOT_ADDRESS: usize = 0x8000;

global_asm!(include_str!("boot.s"),
    ABORT_MODE = const ProcessorMode::Abort as u32,
    SVC_MODE = const ProcessorMode::Supervisor as u32
);

/// Things we do in the first stage:
/// - Set up and enable the MMU
/// - Change the processor mode to system mode
#[unsafe(no_mangle)]
pub extern "C" fn first_stage() -> ! {
    // let mut uart = MiniUart::get().unwrap();
    // uart.set_bit_mode(true);
    // uart.set_baud_rate(115200);
    // let tx_pin: Pin<14, Alternate5> = Pin::get().unwrap();
    // let rx_pin: Pin<15, Alternate5> = Pin::get().unwrap();
    // let mut rx_tx = uart.enable_transmitter(tx_pin).enable_receiver(rx_pin);
    // rx_tx.send_blocking("hello world\n".bytes());

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

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe fn get32(addr: usize) -> u32 {
        core::ptr::read_volatile(addr as *const u32)
    }

    unsafe fn put32(adder: usize, value: u32) {
        core::ptr::write_volatile(adder as *mut u32, value);
    }
    
    const BLINK_DELAY: u32 = 0x400000;
    fn delay(mut n: u32) {
        while n > 0 {
            n -= 1;
            black_box(n);
        }
    }

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
        delay(BLINK_DELAY);

        // Turn on the LED
        unsafe {
            put32(GPCLR1, 1 << 15);
        }
        delay(BLINK_DELAY);
    }
}

// IMPORTANT: You only have 16KiB of stack space. Do not use more than that.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn interrupt_handler() {
    // TODO: ...
}

/// The interrupt handler for Data Abort, Prefecth Abort, and Undefined Instructions.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn interrupt_panic() -> ! {
    panic!()
}
