#![no_std]
#![no_main]

use core::ptr;
use core::task::{Context, RawWaker, RawWakerVTable, Waker};
use core::{arch::global_asm, hint::black_box};

use core::future::Future;
use embassy_time::Timer;
use rpi::mmu::{STACK_TOP, TRANSLATION_TABLE};
use rpi::{data_memory_barrier, interrupt};

global_asm!(
    include_str!("boot.s"),
    TRANSLATION_TABLE = sym TRANSLATION_TABLE,
    ABORT_MODE = const ProcessorMode::Abort as u32,
    ABORT_MODE_STACK = const 0x4000,
    SVC_MODE = const ProcessorMode::Supervisor as u32,
    SYSTEM_MODE = const ProcessorMode::System as u32,
    SYSTEM_MODE_STACK = const STACK_TOP,
);

static NOOP_VTABLE: RawWakerVTable = RawWakerVTable::new(
    |_| RawWaker::new(ptr::null(), &NOOP_VTABLE),
    |_| {},
    |_| {},
    |_| {},
);

#[unsafe(no_mangle)]
pub extern "C" fn first_stage() -> ! {
    // Enable interrupts
    interrupt::setup();
    let mut timer = Timer::after_secs(1);
    let pinned_timer = core::pin::Pin::new(&mut timer);
    let waker = unsafe { Waker::from_raw(RawWaker::new(core::ptr::null(), &NOOP_VTABLE)) };
    let mut context = Context::from_waker(&waker);
    pinned_timer.poll(&mut context);
    
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
    const GPFSEL4: usize = 0x20200010;
    const GPSET1: usize = 0x20200020;
    const GPCLR1: usize = 0x2020002C;
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

/// The interrupt handler for Data Abort, Prefecth Abort, and Undefined Instructions.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn interrupt_panic() -> ! {
    panic!()
}
