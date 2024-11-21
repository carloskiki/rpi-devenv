#![no_std]
#![no_main]

use core::{
    hint::black_box,
    ptr::{read_volatile, write_volatile},
};
use embassy_time::{block_for, Duration};
use rpi::{
    aux::{
        self,
        spi::CsHighTime,
        uart::{BaudRate, BitMode},
    },
    data_memory_barrier,
    eio::Write,
    gpio::{self},
    hal::spi::SpiBus,
    main,
};

#[main]
fn main() -> ! {
    let mut tx = aux::uart::Writer::get(
        gpio::Pin::<14, _>::get().unwrap(),
        &aux::uart::Config {
            baud_rate: BaudRate::new(115200),
            bit_mode: BitMode::EightBits,
        },
    )
    .unwrap();

    let mut spi = aux::spi::Spi1::get(
        gpio::Pin::get().unwrap(),
        gpio::Pin::get().unwrap(),
        gpio::Pin::get().unwrap(),
        &aux::spi::Config {
            speed: aux::spi::Speed::new(499),
            post_input: false,
            data_out_hold: aux::spi::DataOutHold::H0,
            in_rising: false,
            out_rising: false,
            out_most_significant_first: true,
            in_most_significant_first: true,
            extra_cs_high_time: CsHighTime::new(0),
            keep_input: false,
            polarity: rpi::hal::spi::Polarity::IdleHigh,
        },
    )
    .unwrap();


    const SPI_STATUS: *const u8 = 0x20215088 as _;

    let status = unsafe { read_volatile(SPI_STATUS) };
    tx.write_fmt(format_args!("initial Status: 0x{:X}\n", status)).unwrap();
    
    spi.write(&[42_u8, 42, 42, 42]).unwrap();
    let status = unsafe { read_volatile(SPI_STATUS) };
    tx.write_fmt(format_args!("Status after write: 0x{:X}\n", status)).unwrap();

    spi.write(&[46, 46]).unwrap();
    let status = unsafe { read_volatile(SPI_STATUS) };
    tx.write_fmt(format_args!("Status after another write: 0x{:X}\n", status)).unwrap();
    
    spi.write(&[42_u8]).unwrap();
    spi.write(&[42_u8]).unwrap();
    spi.write(&[42_u8]).unwrap();
    let status = unsafe { read_volatile(SPI_STATUS) };
    tx.write_fmt(format_args!("Status after 5 writes: 0x{:X}\n", status)).unwrap();

    data_memory_barrier();
    spi.clear_fifos();

    let status = unsafe { read_volatile(SPI_STATUS) };
    tx.write_fmt(format_args!("Status after clearing fifos: 0x{:X}\n", status)).unwrap();

    loop {}
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
