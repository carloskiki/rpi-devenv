#![no_std]
#![no_main]

use core::{arch::global_asm, hint::black_box, iter::repeat};

use bitflags::bitflags;

mod uart;
mod sync;

// const GPFSEL3: usize = 0x2020000C;
const GPFSEL1: usize = 0x20200004;
const GPFSEL4: usize = 0x20200010;
const GPSET1: usize = 0x20200020;
const GPCLR1: usize = 0x2020002C;

const AUX_ENABLES: usize = 0x20215004;
/// Mini Uart I/O Data
const AUX_MU_IO_REG: usize = 0x20215040;
const AUX_MU_IER_REG: usize = 0x20215044;
/// Mini Uart Interrupt Identify
const AUX_MU_IIR_REG: usize = 0x20215048;
/// Mini Uart Line Control
const AUX_MU_LCR_REG: usize = 0x2021504C;
/// Mini Uart Modem Control
const AUX_MU_MCR_REG: usize = 0x20215050;
/// Mini Uart Line Status
const AUX_MU_LSR_REG: usize = 0x20215054;
/// Mini Uart Modem Status
const AUX_MU_MSR_REG: usize = 0x20215058;
/// Mini Uart Extra Control
const AUX_MU_CNTL_REG: usize = 0x20215060;
/// Mini Uart Extra Status 
const AUX_MU_STAT_REG: usize = 0x20215064;
/// Mini Uart Baudrate
const AUX_MU_BAUD_REG: usize = 0x20215068;

const TIMER_FOUR_SEC: u32 = 0x400000;
const SYS_TIMER_BASE: usize = 0x20003000;
const CLO: usize = SYS_TIMER_BASE + 0x04;

global_asm!(include_str!("boot.s"), options(raw));

extern "C" {
    fn mem_barrier();
}

#[no_mangle]
pub extern "C" fn first_stage() -> ! {
    // Set GPIO pin 47 as output
    unsafe {
        get32(GPFSEL4);
        let mask = 0b111 << 21;
        let output = 0b001 << 21;
        put32(GPFSEL4, (get32(GPFSEL4) & !mask) | output);
    }

    // set GPIO pin 14 to alt function 5
    let func_sel = unsafe { get32(GPFSEL1) };
    let mask = 0b111 << 12;
    let alt_func = 0b010 << 12;
    unsafe {
        put32(GPFSEL1, (func_sel & !mask) | alt_func);
    }

    let mut uart = uart::MiniUart::get().unwrap();
    uart.set_bit_mode(true);
    uart.set_baud_rate(115200);
    let mut transmitter = uart.enable_transmitter();
    transmitter.send_blocking(repeat(b'U'));
    
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

#[export_name = "rust_irq_handler"]
pub extern "C" fn irq_handler() {}

#[repr(u8)]
enum ProcessorMode {
    User = 0b10000,
    Fiq  = 0b10001,
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

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe { mem_barrier() };

    unsafe {
        put32(GPCLR1, 1 << 15);
    }

    loop {}

}


bitflags! {
    pub struct InterruptTable: u64 {
        const AUXILIARY = 1 << 29;
        const I2C_SPI_PERIPHERAL = 1 << 43;
        const SMI = 1 << 48;
        const GPIO_0 = 1 << 49;
        const GPIO_1 = 1 << 50;
        const GPIO_2 = 1 << 51;
        const GPIO_3 = 1 << 52;
        const I2C = 1 << 53;
        const SPI = 1 << 54;
        const PCM = 1 << 55;
        const UART = 1 << 57;

        
        const _ = !0;
    }
}
