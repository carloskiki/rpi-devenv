#![no_std]
#![no_main]

use core::{
    hint::black_box,
    ptr::{read_volatile, write_volatile},
};
use embassy_executor::task;
use rpi::{
    aux::{self},
    data_memory_barrier,
    eio_async::{Read as _, Write as _},
    executor::Executor,
    gpio::{self},
    main,
};

#[main]
fn main() -> ! {
    let mut executor = Executor::new();
    // Safety: We know that the main function never returns.
    let executor: &'static mut Executor = unsafe { core::mem::transmute(&mut executor) };
    executor.run(|spawner| {
        spawner.spawn(async_task()).unwrap();
    })
}

#[task]
async fn async_task() {
    let (mut rx, mut tx) = aux::uart::pair(
        gpio::Pin::<15, _>::get().unwrap(),
        gpio::Pin::<14, _>::get().unwrap(),
        &aux::uart::Config {
            baud_rate: aux::uart::BaudRate::new(115200),
            bit_mode: aux::uart::BitMode::EightBits,
        },
    )
    .unwrap();

    tx.write_all(b"Hello, world!\n").await.unwrap();

    let mut buf = [0; 1];
    loop {
        rx.read_exact(&mut buf).await.unwrap();
        tx.write_all(&buf).await.unwrap();
    }
}

// #[task]
// async fn blocking_task() {
//     let (mut rx, mut tx) = aux::uart::pair(
//         gpio::Pin::<15, _>::get().unwrap(),
//         gpio::Pin::<14, _>::get().unwrap(),
//         &aux::uart::Config {
//             baud_rate: aux::uart::BaudRate::new(115200),
//             bit_mode: aux::uart::BitMode::EightBits,
//         },
//     ).unwrap();
//
//     <aux::uart::writer::Writer<_> as eio::Write>::write_all(&mut tx, b"Hello, world!\n").unwrap();
//
//     let mut buf = [0; 1];
//     loop {
//         <aux::uart::reader::Reader<_> as eio::Read>::read_exact(&mut rx, &mut buf).unwrap();
//         <aux::uart::writer::Writer<_> as eio::Write>::write_all(&mut tx, &buf).unwrap();
//     }
// }

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
