use core::{
    convert::Infallible,
    ptr::{read_volatile, write_volatile},
};

use crate::{
    data_memory_barrier,
    gpio::{state::Alternate5, Pin},
    impl_sealed, Sealed,
};

const CLOCK_SPEED: u32 = 250_000_000;

/// Auxiliary Interrupt status
const AUX_IRQ: *mut u32 = 0x20215000 as _;
/// Auxiliary enables
const AUX_ENABLES: *mut u32 = 0x20215004 as _;
/// Mini Uart I/O Data
const AUX_MU_IO_REG: *mut u32 = 0x20215040 as _;
/// Mini Uart Interrupt Enable
const AUX_MU_IER_REG: *mut u32 = 0x20215044 as _;
/// Mini Uart Interrupt Identify
const AUX_MU_IIR_REG: *mut u32 = 0x20215048 as _;
/// Mini Uart Line Control
const AUX_MU_LCR_REG: *mut u32 = 0x2021504C as _;
/// Mini Uart Modem Control
const AUX_MU_MCR_REG: *mut u32 = 0x20215050 as _;
/// Mini Uart Line Status
const AUX_MU_LSR_REG: *mut u32 = 0x20215054 as _;
/// Mini Uart Modem Status
const AUX_MU_MSR_REG: *mut u32 = 0x20215058 as _;
/// Mini Uart Extra Control
const AUX_MU_CNTL_REG: *mut u32 = 0x20215060 as _;
/// Mini Uart Extra Status
const AUX_MU_STAT_REG: *mut u32 = 0x20215064 as _;
/// Mini Uart Baudrate
const AUX_MU_BAUD_REG: *mut u32 = 0x20215068 as _;

pub struct MiniUart<RxPin, TxPin> {
    baud_rate: u32,
    eight_bits: bool,
    lock: MiniUartLock,
    transmitter_pin: TxPin,
    receiver_pin: RxPin,
}

impl MiniUart<(), ()> {
    pub fn get() -> Option<MiniUart<(), ()>> {
        let lock = MiniUartLock::get()?;
        Some(Self::get_with_lock(lock))
    }

    /// Get the Mini UART without acquiring its lock.
    ///
    /// # Safety
    ///
    /// This is unsafe as you must make sure that this is the only instance of the Mini UART.
    /// Otherwise, the Mini UART will be in an inconsistent state.
    pub unsafe fn get_unchecked() -> Self {
        // Safety: We are in an unsafe function that requires the caller to ensure the conditions
        // for this function.
        let lock = unsafe { MiniUartLock::get_unchecked() };
        Self::get_with_lock(lock)
    }

    fn get_with_lock(lock: MiniUartLock) -> Self {
        // Safety: Only addresses defined in the BCM2835 manual are accessed, and bits are set
        // appropriately. A new `Uart` instance is not created if the peripheral is already in use.
        //
        // Disable the Mini UART RX and TX.
        unsafe { write_volatile(AUX_MU_CNTL_REG, 0) };

        Self {
            baud_rate: 0,
            eight_bits: false,
            lock,
            transmitter_pin: (),
            receiver_pin: (),
        }
    }
}

impl<RxPin, TxPin> MiniUart<RxPin, TxPin> {
    pub fn set_baud_rate(&mut self, baud_rate: u32) {
        assert!(
            (476..=31_250_000).contains(&baud_rate),
            "baud rate not in the range 476..=31_250_000"
        );
        let baud_rate_reg = (CLOCK_SPEED / (8 * baud_rate)) - 1;
        // Safety: Only addresses defined in the BCM2835 manual are accessed, and bits are set
        // appropriately.
        //
        // The input baud rate is in the range 476..=31_250_000, therefore baud_rate_reg is a valid
        // u16 value.
        unsafe {
            write_volatile(AUX_MU_BAUD_REG, baud_rate_reg);
        }
        self.baud_rate = baud_rate;
    }

    pub fn set_bit_mode(&mut self, eight_bits: bool) {
        // Safety: Only addresses defined in the BCM2835 manual are accessed, and bits are set
        // appropriately.
        unsafe {
            write_volatile(AUX_MU_LCR_REG, if eight_bits { 3 } else { 0 });
        }
        self.eight_bits = eight_bits;
    }

    pub fn baud_rate(&self) -> u32 {
        self.baud_rate
    }

    pub fn eight_bits(&self) -> bool {
        self.eight_bits
    }
}

impl<RxPin, TxPin> MiniUart<RxPin, TxPin>
where
    RxPin: MiniUartRxPin<Enabled = False>,
    TxPin: MiniUartTxPin,
{
    /// Enable the Mini UART receiver without providing a valid pin.
    ///
    /// # Safety
    ///
    /// The caller must ensure that a Mini UART receiver pin is properly configured in order to
    /// receive data.
    #[allow(private_interfaces)]
    pub unsafe fn enable_receiver_no_pin(self) -> MiniUart<UnsafeRxPin, TxPin> {
        // Safety: Only addresses defined in the BCM2835 manual are accessed, and bits are set
        // appropriately. A memory barrier is used according to the BCM2835 manual section 1.3.
        unsafe {
            write_volatile(AUX_MU_CNTL_REG, ((TxPin::ENABLED as u32) << 1) | 1);
            write_volatile(AUX_MU_IIR_REG, 0b10);
        }

        MiniUart {
            baud_rate: self.baud_rate,
            eight_bits: self.eight_bits,
            lock: self.lock,
            transmitter_pin: self.transmitter_pin,
            receiver_pin: UnsafeRxPin,
        }
    }

    pub fn enable_receiver<const N: u8>(
        self,
        pin: Pin<N, Alternate5>,
    ) -> MiniUart<Pin<N, Alternate5>, TxPin>
    where
        Pin<N, Alternate5>: MiniUartRxPin,
    {
        // Safety: We have a valid pin, so we can safely call `enable_receiver_no_pin`.
        let rx_enabled = unsafe { self.enable_receiver_no_pin() };
        MiniUart {
            baud_rate: rx_enabled.baud_rate,
            eight_bits: rx_enabled.eight_bits,
            lock: rx_enabled.lock,
            transmitter_pin: rx_enabled.transmitter_pin,
            receiver_pin: pin,
        }
    }
}

impl<RxPin, TxPin> MiniUart<RxPin, TxPin>
where
    RxPin: MiniUartRxPin,
    TxPin: MiniUartTxPin<Enabled = False>,
{
    /// Enable the Mini UART transmitter without providing a valid pin.
    ///
    /// # Safety
    ///
    /// The caller must ensure that a Mini UART transmitter pin is properly configured in order
    /// to send data.
    #[allow(private_interfaces)]
    pub unsafe fn enable_transmitter_no_pin(self) -> MiniUart<RxPin, UnsafeTxPin> {
        // Safety: Only addresses defined in the BCM2835 manual are accessed, and bits are set
        // appropriately.
        unsafe {
            write_volatile(AUX_MU_CNTL_REG, 0b10 | RxPin::ENABLED as u32);
            write_volatile(AUX_MU_IIR_REG, 0b100);
        }
        MiniUart {
            baud_rate: self.baud_rate,
            eight_bits: self.eight_bits,
            lock: self.lock,
            transmitter_pin: UnsafeTxPin,
            receiver_pin: self.receiver_pin,
        }
    }

    /// Enable the Mini UART transmitter with a valid pin.
    pub fn enable_transmitter<const N: u8>(
        self,
        pin: Pin<N, Alternate5>,
    ) -> MiniUart<RxPin, Pin<N, Alternate5>>
    where
        Pin<N, Alternate5>: MiniUartTxPin,
    {
        // Safety: We have a valid pin, so we can safely call `enable_transmitter_no_pin`.
        let tx_enabled = unsafe { self.enable_transmitter_no_pin() };
        MiniUart {
            baud_rate: tx_enabled.baud_rate,
            eight_bits: tx_enabled.eight_bits,
            lock: tx_enabled.lock,
            transmitter_pin: pin,
            receiver_pin: tx_enabled.receiver_pin,
        }
    }
}

impl<RxPin, TxPin> MiniUart<RxPin, TxPin>
where
    RxPin: MiniUartRxPin<Enabled = True>,
    TxPin: MiniUartTxPin,
{
    /// Receive data from the Mini UART.
    ///
    /// This method will not block, so as little as 0 bytes may be received, and as many as the
    /// buffer can hold. The number of bytes received is returned.
    pub fn receive(&mut self, buf: &mut [u8]) -> usize {
        data_memory_barrier();
        for (count, byte) in buf.iter_mut().enumerate() {
            // Safety: Only addresses defined in the BCM2835 manual are accessed.
            //  Memory barriers are used according to the BCM2835 manual section 1.3.
            if unsafe { read_volatile(AUX_MU_LSR_REG) } & 1 == 0 {
                return count;
            }
            // Safety: Only addresses defined in the BCM2835 manual are accessed.
            *byte = unsafe { read_volatile(AUX_MU_IO_REG) as u8 };
        }
        buf.len()
    }

    /// Receive exactly `buf.len()` bytes from the Mini UART.
    ///
    /// This method will block until `buf.len()` bytes are received.
    pub fn receive_exact(&mut self, buf: &mut [u8]) {
        data_memory_barrier();
        // Safety: Memory barriers are used according to the BCM2835 manual section 1.3.
        for byte in buf {
            // Safety: Only addresses defined in the BCM2835 manual are accessed.
            while unsafe { read_volatile(AUX_MU_LSR_REG) } & 1 == 0 {}
            // Safety: Only addresses defined in the BCM2835 manual are accessed.
            *byte = unsafe { read_volatile(AUX_MU_IO_REG) as u8 };
        }
    }

    /// Disable the Mini UART receiver.
    ///
    /// Drops the receiver pin if it was used.
    pub fn disable_receiver(self) -> MiniUart<(), TxPin> {
        data_memory_barrier();
        // Safety: Only addresses defined in the BCM2835 manual are accessed.
        unsafe {
            write_volatile(AUX_MU_CNTL_REG, (TxPin::ENABLED as u32) << 1);
        }
        MiniUart {
            baud_rate: self.baud_rate,
            eight_bits: self.eight_bits,
            lock: self.lock,
            transmitter_pin: self.transmitter_pin,
            receiver_pin: (),
        }
    }
}

impl<RxPin, TxPin> MiniUart<RxPin, TxPin>
where
    RxPin: MiniUartRxPin,
    TxPin: MiniUartTxPin<Enabled = True>,
{
    pub fn send_blocking(&mut self, bytes: impl IntoIterator<Item = u8>) {
        data_memory_barrier();
        for byte in bytes {
            // Safety: Only addresses defined in the BCM2835 manual are accessed.
            //  Memory barriers are used according to the BCM2835 manual section 1.3.
            unsafe {
                while read_volatile(AUX_MU_LSR_REG) & 0x20 == 0 {}
                write_volatile(AUX_MU_IO_REG, byte as u32);
            }
        }
    }

    pub fn send(&mut self, bytes: &mut impl Iterator<Item = u8>) -> usize {
        let mut sent = 0;
        data_memory_barrier();
        for byte in bytes {
            // Safety: Only addresses defined in the BCM2835 manual are accessed.
            //  Memory barriers are used according to the BCM2835 manual section 1.3.
            unsafe {
                if read_volatile(AUX_MU_LSR_REG) & 0x20 == 0 {
                    break;
                }
                write_volatile(AUX_MU_IO_REG, byte as u32);
                sent += 1;
            }
        }
        sent
    }

    /// Disable the Mini UART transmitter.
    ///
    /// Drops the transmitter pin if it was used.
    pub fn disable_transmitter(self) -> MiniUart<RxPin, ()> {
        // Safety: Only addresses defined in the BCM2835 manual are accessed, and bits are set
        //  appropriately.
        unsafe {
            write_volatile(AUX_MU_CNTL_REG, RxPin::ENABLED as u32);
        }
        MiniUart {
            baud_rate: self.baud_rate,
            eight_bits: self.eight_bits,
            lock: self.lock,
            transmitter_pin: (),
            receiver_pin: self.receiver_pin,
        }
    }
}

// This exists because you can't destruct structs that `impl Drop`
struct MiniUartLock;

impl MiniUartLock {
    fn get() -> Option<Self> {
        data_memory_barrier();
        // Safety: Only addresses defined in the BCM2835 manual are accessed, and bits are set
        // appropriately. A new `Uart` instance is not created if the peripheral is already in use.
        // A memory barrier is used according to the BCM2835 manual section 1.3.
        unsafe {
            let enable_state = read_volatile(AUX_ENABLES);
            if enable_state & 1 != 0 {
                return None;
            }
            write_volatile(AUX_ENABLES, enable_state | 1);
        }
        Some(Self)
    }

    /// Get the Mini UART lock without checking if it is already in use.
    ///
    /// # Safety
    ///
    /// This is unsafe as you must make sure that this is the only instance of the Mini UART.
    unsafe fn get_unchecked() -> Self {
        data_memory_barrier();
        // Safety: Only addresses defined in the BCM2835 manual are accessed, and bits are set
        // appropriately. A memory barrier is used according to the BCM2835 manual section 1.3.
        unsafe {
            let enable_state = read_volatile(AUX_ENABLES);
            write_volatile(AUX_ENABLES, enable_state | 1);
        }

        Self
    }
}

impl Drop for MiniUartLock {
    fn drop(&mut self) {
        data_memory_barrier();
        // Safety: Only addresses defined in the BCM2835 manual are accessed, and bits are set
        // appropriately. A memory barrier is used according to the BCM2835 manual section 1.3.
        unsafe {
            let enable_state = read_volatile(AUX_ENABLES);
            write_volatile(AUX_ENABLES, enable_state & !1);
        }
    }
}

struct UnsafeRxPin;
struct UnsafeTxPin;
impl_sealed!(UnsafeRxPin, UnsafeTxPin);

type True = ();
type False = Infallible;

/// GPIO [`Pin`]s that can be used for the Mini UART as a receiver.
#[allow(private_bounds)]
pub trait MiniUartRxPin: Sealed {
    type Enabled;
    const ENABLED: bool = true;
}
/// GPIO [`Pin`]s that can be used for the Mini UART as a transmitter.
#[allow(private_bounds)]
pub trait MiniUartTxPin: Sealed {
    type Enabled;
    const ENABLED: bool = true;
}

// See the BCM2835 manual section 6.2 for the pin mappings.
impl MiniUartRxPin for Pin<15, Alternate5> {
    type Enabled = True;
}
impl MiniUartRxPin for Pin<33, Alternate5> {
    type Enabled = True;
}
impl MiniUartRxPin for Pin<41, Alternate5> {
    type Enabled = True;
}
impl MiniUartRxPin for UnsafeRxPin {
    type Enabled = True;
}
impl MiniUartRxPin for () {
    type Enabled = False;
    const ENABLED: bool = false;
}

impl MiniUartTxPin for Pin<14, Alternate5> {
    type Enabled = True;
}
impl MiniUartTxPin for Pin<32, Alternate5> {
    type Enabled = True;
}
impl MiniUartTxPin for Pin<40, Alternate5> {
    type Enabled = True;
}
impl MiniUartTxPin for UnsafeTxPin {
    type Enabled = True;
}
impl MiniUartTxPin for () {
    type Enabled = False;
    const ENABLED: bool = false;
}
