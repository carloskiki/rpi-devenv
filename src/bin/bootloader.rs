#![no_std]
#![no_main]

use rpi::gpio::{Alternate5, Pin};

core::arch::global_asm!(include_str!("bootloader/boot.s"), options(raw));

#[no_mangle]
pub extern "C" fn first_stage() -> ! {
    // Safety: QEMU the bitch
    // let mut uart = unsafe { rpi::uart::MiniUart::get_unchecked() };
    // uart.set_bit_mode(true);
    // uart.set_baud_rate(115200);
    // let tx_pin: Pin<14, Alternate5> = Pin::get().unwrap();
    // let rx_pin: Pin<15, Alternate5> = Pin::get().unwrap();
    // let mut rx_tx = uart.enable_transmitter(tx_pin).enable_receiver(rx_pin);
    // rx_tx.send_blocking("hello world\n".bytes());
    // let mut byte = [0];
    // loop {
    //     rx_tx.receive_exact(&mut byte[..]);
    //     rx_tx.send_blocking(byte.iter().copied());
    // }
    loop {}
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
