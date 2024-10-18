#![no_std]
#![no_main]
use rpi::gpio::{Alternate5, Pin};

core::arch::global_asm!(include_str!("boot.s"), options(raw));

#[no_mangle]
pub extern "C" fn first_stage() -> ! {
    let mut uart = rpi::uart::MiniUart::get().unwrap();
    uart.set_bit_mode(true);
    uart.set_baud_rate(115200);
    let tx_pin: Pin<14, Alternate5> = Pin::get().unwrap();
    let rx_pin: Pin<15, Alternate5> = Pin::get().unwrap();
    let mut rx_tx = uart.enable_transmitter(tx_pin).enable_receiver(rx_pin);
    rx_tx.send_blocking([0xff].into_iter());

    let mut binary_size_bytes = [0u8; 4];
    rx_tx.receive_exact(&mut binary_size_bytes);
    let mut binary_size = u32::from_le_bytes(binary_size_bytes);

    let mut byte_buf = [0u8];
    // Safety: This value is provided by the linker script.
    unsafe extern "C" {
        #[link_name = "__physical_load_address"]
        static LOAD_ADDRESS: u8;
    }
    // Safety: We know that there is nothing at __physical_load_address, since we relocated to
    // 0x2000000 (__relocate_address).
    unsafe {
        let mut dest_ptr: *mut u8 = (&raw const LOAD_ADDRESS) as _;
        while binary_size > 0 {
            rx_tx.receive_exact(&mut byte_buf);

            dest_ptr.write(byte_buf[0]);
            dest_ptr = dest_ptr.offset(1);
            binary_size -= 1;
        }
    }

    // Clean up before jumping to the kernel
    drop(rx_tx);

    // Safety: We know that the kernel is a function that never returns.
    // We also have loaded it into memory at LOAD_ADDRESS.
    unsafe {
        let kernel: fn() -> ! = core::mem::transmute(&raw const LOAD_ADDRESS);
        kernel();
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
