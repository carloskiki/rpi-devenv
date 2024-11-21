#![no_std]
#![no_main]

use core::{
    hint::black_box,
    ptr::{read_volatile, write_volatile},
};
use rpi::{
    aux::{
        self,
        uart::{BaudRate, BitMode},
    },
    data_memory_barrier,
    eio::Write,
    gpio::{self},
    main,
};

#[main]
fn main() -> ! {
    let mut tx = unsafe {
        aux::uart::Writer::get_unchecked(
            gpio::Pin::<14, _>::get().unwrap(),
            &aux::uart::Config {
                baud_rate: BaudRate::new(115200),
                bit_mode: BitMode::EightBits,
            },
        )
    };

    const AUX_ENABLES: *mut u32 = 0x20215004 as _;
    const SPI_CNTL0: *mut u32 = 0x20215080 as _;
    const SPI_CNTL1: *mut u32 = 0x20215084 as _;
    const SPI_STAT: *mut u32 = 0x20215088 as _;
    const SPI_IO: *mut u32 = 0x202150A0 as _;
    const SPI_TXHOLD: *mut u32 = 0x202150B0 as _;

    // Enable the SPI peripheral
    unsafe { write_volatile(AUX_ENABLES, read_volatile(AUX_ENABLES) | 0b10) };
    let speed = 4999;
    let chip_select = 1;
    let cntl0 = (speed << 20) | (chip_select << 17) | (0b11 << 14) | (1 << 11) | 0b100;
    unsafe { write_volatile(SPI_CNTL0, cntl0) };
    unsafe { SPI_IO.write_volatile(8 << 24) };
    let status = unsafe { SPI_STAT.read_volatile() };
    tx.write_fmt(format_args!("SPI_STAT: {:#010X}\n", status))
        .unwrap();
    tx.write_fmt(format_args!("tx level: {}\n", status >> 28))
        .unwrap();

    let cntl0 = unsafe { SPI_CNTL0.read_volatile() };
    tx.write_fmt(format_args!("SPI_CNTL0: {:#010X}\n", cntl0))
        .unwrap();
    unsafe { SPI_CNTL0.write_volatile(cntl0 | 1 << 9) };
    let cntl0 = unsafe { SPI_CNTL0.read_volatile() };
    tx.write_fmt(format_args!("SPI_CNTL0: {:#010X}\n", cntl0))
        .unwrap();
    unsafe { SPI_CNTL0.write_volatile(cntl0 | !(1 << 9)) };

    let status = unsafe { SPI_STAT.read_volatile() };
    tx.write_fmt(format_args!("SPI_STAT: {:#010X}\n", status))
        .unwrap();
    tx.write_fmt(format_args!("rx level: {}\n", status >> 20))
        .unwrap();

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
