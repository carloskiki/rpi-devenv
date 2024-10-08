use crate::{get32, gpio::{Alternate5, Pin}, mem_barrier, put32};

const CLOCK_SPEED: u32 = 250_000_000;

/// Auxiliary Interrupt status
const AUX_IRQ: usize = 0x20215000;
/// Auxiliary enables
const AUX_ENABLES: usize = 0x20215004;
/// Mini Uart I/O Data
const AUX_MU_IO_REG: usize = 0x20215040;
/// Mini Uart Interrupt Enable
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

pub struct MiniUart<const RX_ENABLE: bool, const TX_ENABLE: bool>{
    baud_rate: u32,
    eight_bits: bool,
    lock: MiniUartLock,
    transmitter_pin: Option<Pin<14, Alternate5>>,
    receiver_pin: Option<Pin<15, Alternate5>>,
}

impl MiniUart<false, false> {
    pub fn get() -> Option<Self> {
        let lock = MiniUartLock::get()?;
        // Safety: Only addresses defined in the BCM2835 manual are accessed, and bits are set
        // appropriately. A new `Uart` instance is not created if the peripheral is already in use.
        // A memory barrier is used according to the BCM2835 manual section 1.3.
        //
        // Disable the Mini UART RX and TX.
        unsafe { put32(AUX_MU_CNTL_REG, 0) };
        
        Some(Self {
            baud_rate: 0,
            eight_bits: false,
            lock,
            transmitter_pin: None,
            receiver_pin: None,
        })
    }

    /// Get the Mini UART without acquiring its lock.
    /// 
    /// This is unsafe as you must make sure that this is the only instance of the Mini UART.
    /// Otherwise, the Mini UART will be in an inconsistent state. _Even if unused, having
    /// multiple instances of the Mini UART will cause undefined behaviour._
    pub unsafe fn get_unlocked() -> Self {
        // Safety: Only addresses defined in the BCM2835 manual are accessed, and bits are set
        // appropriately. A memory barrier is used according to the BCM2835 manual section 1.3.
        //
        // Disable the Mini UART RX and TX.
        unsafe { put32(AUX_MU_CNTL_REG, 0) };
        
        Self {
            baud_rate: 0,
            eight_bits: false,
            lock: MiniUartLock,
            transmitter_pin: None,
            receiver_pin: None,
        }
    }

    pub fn set_baud_rate(&mut self, baud_rate: u32) {
        assert!((476..=31_250_000).contains(&baud_rate), "baud rate not in the range 476..=31_250_000");
        let baud_rate_reg = (CLOCK_SPEED / (8 * baud_rate)) - 1;
        // Safety: Only addresses defined in the BCM2835 manual are accessed, and bits are set
        // appropriately. A memory barrier is used according to the BCM2835 manual section 1.3.
        //
        // The input baud rate is in the range 476..=31_250_000, therefore baud_rate_reg is a valid
        // u16 value.
        unsafe {
            mem_barrier();
            put32(AUX_MU_BAUD_REG, baud_rate_reg);
        }
        self.baud_rate = baud_rate;
    }

    pub fn set_bit_mode(&mut self, eight_bits: bool) {
        // Safety: Only addresses defined in the BCM2835 manual are accessed, and bits are set
        // appropriately. A memory barrier is used according to the BCM2835 manual section 1.3.
        unsafe {
            mem_barrier();
            put32(AUX_MU_LCR_REG, if eight_bits { 3 } else { 0 });
        }
        self.eight_bits = eight_bits;
    }
}

impl<const TX_ENABLE: bool> MiniUart<false, TX_ENABLE> {
    /// Enable the Mini UART receiver without providing a valid pin.
    ///
    /// # Safety
    ///
    /// The caller must ensure that a Mini UART receiver pin is properly configured in order to
    /// receive data.
    pub unsafe fn enable_receiver_no_pin(self) -> MiniUart<true, TX_ENABLE> {
        // Safety: Only addresses defined in the BCM2835 manual are accessed, and bits are set
        // appropriately. A memory barrier is used according to the BCM2835 manual section 1.3.
        unsafe {
            mem_barrier();
            put32(AUX_MU_CNTL_REG, ((TX_ENABLE as u32) << 1) | 1);
            put32(AUX_MU_IIR_REG, 0b10);
        }
        MiniUart {
            baud_rate: self.baud_rate,
            eight_bits: self.eight_bits,
            lock: self.lock,
            transmitter_pin: self.transmitter_pin,
            receiver_pin: None,
        }
    }

    pub fn enable_receiver(self, pin: Pin<15, Alternate5>) -> MiniUart<true, TX_ENABLE> {
        // Safety: We have a valid pin, so we can safely call `enable_receiver_no_pin`.
        let mut rx_enabled = unsafe { self.enable_receiver_no_pin() };
        rx_enabled.receiver_pin = Some(pin);
        rx_enabled
    }
}

impl<const RX_ENABLE: bool> MiniUart<RX_ENABLE, false> {
    /// Enable the Mini UART transmitter without providing a valid pin.
    ///
    /// # Safety
    ///
    /// The caller must ensure that a Mini UART transmitter pin is properly configured in order
    /// to send data.
    pub unsafe fn enable_transmitter_no_pin(self) -> MiniUart<RX_ENABLE, true> {
        // Safety: Only addresses defined in the BCM2835 manual are accessed, and bits are set
        // appropriately. A memory barrier is used according to the BCM2835 manual section 1.3.
        unsafe {
            mem_barrier();
            put32(AUX_MU_CNTL_REG, 0b10 | RX_ENABLE as u32);
            put32(AUX_MU_IIR_REG, 0b100);
        }
        MiniUart {
            baud_rate: self.baud_rate,
            eight_bits: self.eight_bits,
            lock: self.lock,
            transmitter_pin: None,
            receiver_pin: self.receiver_pin,
        }
    }

    pub fn enable_transmitter(self, pin: Pin<14, Alternate5>) -> MiniUart<RX_ENABLE, true> {
        // Safety: We have a valid pin, so we can safely call `enable_transmitter_no_pin`.
        let mut tx_enabled = unsafe { self.enable_transmitter_no_pin() };
        tx_enabled.transmitter_pin = Some(pin);
        tx_enabled
    }
}

impl<const TX_ENABLE: bool> MiniUart<true, TX_ENABLE> {
    pub fn receive(&mut self, buf: &mut [u8]) -> usize {
        for (i, byte) in buf.iter_mut().enumerate() {
            if unsafe { get32(AUX_MU_LSR_REG) } & 1 == 0 {
                return i;
            }
            *byte = unsafe { get32(AUX_MU_IO_REG) as u8 };
        }
        buf.len()
    }

    pub fn receive_exact(&mut self, buf: &mut [u8]) {
        for byte in buf {
            while unsafe { get32(AUX_MU_LSR_REG) } & 1 == 0 {}
            *byte = unsafe { get32(AUX_MU_IO_REG) as u8 };
        }
    }

    /// Disable the Mini UART receiver.
    ///
    /// Drops the receiver pin if it was used.
    pub fn disable_receiver(self) -> MiniUart<false, TX_ENABLE> {
        // Safety: Only addresses defined in the BCM2835 manual are accessed, and bits are set
        // appropriately. A memory barrier is used according to the BCM2835 manual section 1.3.
        unsafe {
            mem_barrier();
            put32(AUX_MU_CNTL_REG, (TX_ENABLE as u32) << 1);
        }
        MiniUart {
            baud_rate: self.baud_rate,
            eight_bits: self.eight_bits,
            lock: self.lock,
            transmitter_pin: self.transmitter_pin,
            receiver_pin: None,
        }
    }
}

impl<const RX_ENABLE: bool> MiniUart<RX_ENABLE, true> {
    pub fn send_blocking(&mut self, bytes: impl Iterator<Item = u8>) {
        // Safety: Only addresses defined in the BCM2835 manual are accessed, and bits are set
        // appropriately. A memory barrier is used according to the BCM2835 manual section 1.3.
        unsafe { mem_barrier() }; 
        
        for byte in bytes {
            unsafe {
                while get32(AUX_MU_LSR_REG) & 0x20 == 0 {}
                put32(AUX_MU_IO_REG, byte as u32);
            }
        }
    }

    pub fn send(&mut self, bytes: &mut impl Iterator<Item = u8>) -> usize {
        // Safety: Only addresses defined in the BCM2835 manual are accessed, and bits are set
        // appropriately. A memory barrier is used according to the BCM2835 manual section 1.3.
        let mut sent = 0;
        unsafe { mem_barrier() };
        for byte in bytes {
            unsafe {
                if get32(AUX_MU_LSR_REG) & 0x20 == 0 {
                    break;
                }
                put32(AUX_MU_IO_REG, byte as u32);
                sent += 1;
            }
        }
        sent
    }

    /// Disable the Mini UART transmitter.
    ///
    /// Drops the transmitter pin if it was used.
    pub fn disable_transmitter(self) -> MiniUart<RX_ENABLE, false> {
        // Safety: Only addresses defined in the BCM2835 manual are accessed, and bits are set
        // appropriately. A memory barrier is used according to the BCM2835 manual section 1.3.
        unsafe {
            mem_barrier();
            put32(AUX_MU_CNTL_REG, RX_ENABLE as u32);
        }
        MiniUart {
            baud_rate: self.baud_rate,
            eight_bits: self.eight_bits,
            lock: self.lock,
            transmitter_pin: None,
            receiver_pin: self.receiver_pin,
        }
    }
}
    
impl<const RX_ENABLE: bool, const TX_ENABLE: bool> MiniUart<RX_ENABLE, TX_ENABLE> {
    pub fn baud_rate(&self) -> u32 {
        self.baud_rate
    }

    pub fn eight_bits(&self) -> bool {
        self.eight_bits
    }
}

struct MiniUartLock;

// This exists because you can't destruct structs that `impl Drop`
impl MiniUartLock {
    fn get() -> Option<Self> {
        // Safety: Only addresses defined in the BCM2835 manual are accessed, and bits are set
        // appropriately. A new `Uart` instance is not created if the peripheral is already in use.
        // A memory barrier is used according to the BCM2835 manual section 1.3.
        unsafe {
            let enable_state = get32(AUX_ENABLES);
            if enable_state & 1 != 0 {
                return None;
            }
            mem_barrier();
            put32(AUX_ENABLES, enable_state | 1);
        }
        Some(Self)
    }
}

impl Drop for MiniUartLock {
    fn drop(&mut self) {
        // Safety: Only addresses defined in the BCM2835 manual are accessed, and bits are set
        // appropriately. A memory barrier is used according to the BCM2835 manual section 1.3.
        unsafe {
            let enable_state = get32(AUX_ENABLES);
            mem_barrier();
            put32(AUX_ENABLES, enable_state & !1);
        }
    }
}

// Notice about memory ordering: 
// The BCM2835 system uses an AMBA AXI-compatible interface structure. In order to keep
// the system complexity low and data throughput high, the BCM2835 AXI system does not
// always return read data in-order2
// . The GPU has special logic to cope with data arriving outof-order; however the ARM core does not contain such logic.
// Therefore some precautions must be taken when using the ARM to access peripherals.
// Accesses to the same peripheral will always arrive and return in-order. It is only when
// switching from one peripheral to another that data can arrive out-of-order. The simplest way
// to make sure that data is processed in-order is to place a memory barrier instruction at critical
// positions in the code. You should place:
// • A memory write barrier before the first write to a peripheral.
// • A memory read barrier after the last read of a peripheral.
// It is not required to put a memory barrier instruction after each read or write access. Only at
// those places in the code where it is possible that a peripheral read or write may be followed
// by a read or write of a different peripheral. This is normally at the entry and exit points of the
// peripheral service code.
// As interrupts can appear anywhere in the code so you should safeguard those. If an interrupt
// routine reads from a peripheral the routine should start with a memory read barrier. If an
// interrupt routine writes to a peripheral the routine should end with a memory write barrier. 
