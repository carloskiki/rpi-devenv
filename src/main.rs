#![no_std]
#![no_main]

use core::{arch::global_asm, hint::black_box};

use rpi::mem_barrier;
use rpi::uart::MiniUart;
use rpi::gpio::{Alternate5, Pin};

// const GPFSEL3: usize = 0x2020000C;
const GPFSEL4: usize = 0x20200010;
const GPSET1: usize = 0x20200020;
const GPCLR1: usize = 0x2020002C;

const TIMER_FOUR_SEC: u32 = 0x400000;

global_asm!(include_str!("boot.s"), options(raw));

#[no_mangle]
pub extern "C" fn first_stage() -> ! {
    // Set GPIO pin 47 as output
    unsafe {
        get32(GPFSEL4);
        let mask = 0b111 << 21;
        let output = 0b001 << 21;
        put32(GPFSEL4, (get32(GPFSEL4) & !mask) | output);
    }

    // Turn on the LED
    unsafe {
        put32(GPCLR1, 1 << 15);
    }

    // Safety: QEMU being a bitch
    let mut uart = unsafe { MiniUart::get_unlocked() };
    uart.set_bit_mode(true);
    uart.set_baud_rate(115200);
    let tx_pin: Pin<14, Alternate5> = Pin::get().unwrap();
    let rx_pin: Pin<15, Alternate5> = Pin::get().unwrap();
    let mut rx_tx = uart
        .enable_transmitter(tx_pin)
        .enable_receiver(rx_pin);
    rx_tx.send_blocking("hello world\n".bytes());
    let mut byte = [0];
    loop {
        rx_tx.receive_exact(&mut byte[..]);
        rx_tx.send_blocking(byte.iter().copied());
    }
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

#[export_name = "rust_irq_handler"]
pub extern "C" fn irq_handler() {}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe { mem_barrier() };

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

