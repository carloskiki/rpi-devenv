#![no_std]
#![no_main]

use core::{
    hint::black_box,
    ptr::{read_volatile, write_volatile},
};
use embassy_executor::task;
use embassy_time::{block_for, Duration, Timer};
use rpi::{
    data_memory_barrier,
    executor::Executor,
    gpio::{
        self,
        state::{Alternate5, Output},
        Pin,
    },
    hal::digital::OutputPin,
    main,
};

#[main]
fn main() -> ! {
    let mut executor = Executor::new();
    // Safety: We know that the main function never returns.
    let executor: &'static mut Executor = unsafe { core::mem::transmute(&mut executor) };
    executor.run(|spawner| {
        spawner.spawn(task()).unwrap();
    })
}

#[task]
async fn task() {
    let mut led: gpio::Pin<47, Output> =
        gpio::Pin::get().expect("The pin should not be used anywhere else");

    loop {
        led.set_low().unwrap();
        Timer::after(Duration::from_secs(1)).await;
        led.set_high().unwrap();
    }
}

// Blink the led on panic
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    const BLINK_DELAY: u32 = 0x4000000;
    const GPFSEL4: *mut u32 = 0x20200010 as _;
    const GPSET1: *mut u32 = 0x20200020 as _;
    const GPCLR1: *mut u32 = 0x2020002C as _;
    fn delay(mut n: u32) {
        while n > 0 {
            n -= 1;
            black_box(n);
        }
    }

    data_memory_barrier();
    // Set GPIO pin 47 as output
    unsafe {
        read_volatile(GPFSEL4);
        let mask = 0b111 << 21;
        let output = 0b001 << 21;
        write_volatile(GPFSEL4, (read_volatile(GPFSEL4) & !mask) | output);
    }

    loop {
        // Turn off the LED
        unsafe {
            write_volatile(GPSET1, 1 << 15);
        }
        delay(BLINK_DELAY);

        // Turn on the LED
        unsafe {
            write_volatile(GPCLR1, 1 << 15);
        }
        delay(BLINK_DELAY);
    }
}
