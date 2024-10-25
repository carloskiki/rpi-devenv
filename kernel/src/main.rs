#![no_std]
#![no_main]

use core::{arch::global_asm, hint::black_box};

use rpi::gpio::{Alternate5, Pin};
use rpi::mmu::{enable_mmu, STACK_TOP};
use rpi::uart::MiniUart;
use rpi::data_memory_barrier;

// const GPFSEL3: usize = 0x2020000C;
const GPFSEL4: usize = 0x20200010;
const GPSET1: usize = 0x20200020;
const GPCLR1: usize = 0x2020002C;

global_asm!(include_str!("boot.s"),
    ABORT_MODE = const ProcessorMode::Abort as u32,
    SVC_MODE = const ProcessorMode::Supervisor as u32,
    SYSTEM_MODE = const ProcessorMode::System as u32,
    SYSTEM_MODE_STACK = const STACK_TOP,
);

unsafe extern "C" {
    // Switch to system mode, and enable all interrupts.
    fn to_system_mode();
}

/// Things we do in the first stage:
/// - Set up and enable the MMU
/// - Change the processor mode to system mode
#[unsafe(no_mangle)]
pub extern "C" fn first_stage() -> ! {
    // Safety: QEMU is a bitch
    let mut uart = unsafe { MiniUart::get_unchecked() };
    uart.set_bit_mode(true);
    uart.set_baud_rate(115200);
    let tx_pin: Pin<14, Alternate5> = Pin::get().unwrap();
    let rx_pin: Pin<15, Alternate5> = Pin::get().unwrap();
    let mut rx_tx = uart.enable_transmitter(tx_pin).enable_receiver(rx_pin);
    rx_tx.send_blocking("hello world\n".bytes());

    enable_mmu();

    // Safety: We know that this routine provided by our boot code is OK
    unsafe { to_system_mode() };

    rx_tx.send_blocking("MMU enabled\n".bytes());

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
