use core::{
    convert::Infallible,
    future::Future,
    ptr::{read_volatile, write_volatile},
    slice::IterMut,
    task::Poll,
};

use crate::{
    data_memory_barrier,
    gpio::{state::Alternate5, Pin},
    impl_sealed, set_waker, Sealed, WakerCell, WAKER_CELL_INIT,
};

use embedded_hal_nb as hal_nb;
use embedded_io::{self as eio, ReadExactError};
use embedded_io_async as eio_async;

// This is the clock speed of the "system clock," which is the VPU clock (Video Core).
const CLOCK_SPEED: u32 = 250_000_000;

/// Auxiliary Interrupt status
/// BCM2835 ARM Peripherals, page 9
const AUX_INTERRUPT_STATUS: *mut u32 = 0x20215000 as _;
/// Auxiliary enables
/// BCM2835 ARM Peripherals, page 9
const AUX_ENABLES: *mut u32 = 0x20215004 as _;
/// Mini Uart I/O Data
/// BCM2835 ARM Peripherals, page 11
const IO_REG: *mut u32 = 0x20215040 as _;
/// Mini Uart Interrupt Enable
/// BCM2835 ARM Peripherals, page 12
const INTERRUPT_ENABLE_REG: *mut u32 = 0x20215044 as _;
/// Mini Uart Interrupt Identify
/// BCM2835 ARM Peripherals, page 13
const INTERRUPT_ID_REG: *mut u32 = 0x20215048 as _;
/// Mini Uart Line Control
/// BCM2835 ARM Peripherals, page 14
const LINE_CONTROL_REG: *mut u32 = 0x2021504C as _;
/// Mini Uart Modem Control
/// BCM2835 ARM Peripherals, page 14
const MODEM_CONTROL_REG: *mut u32 = 0x20215050 as _;
/// Mini Uart Line Status
/// BCM2835 ARM Peripherals, page 15
const LINE_STATUS_REG: *mut u32 = 0x20215054 as _;
/// Mini Uart Modem Status
/// BCM2835 ARM Peripherals, page 15
const MODEM_STATUS_REG: *mut u32 = 0x20215058 as _;
/// Mini Uart Extra Control
/// BCM2835 ARM Peripherals, page 16
const EXTRA_CONTROL_REG: *mut u32 = 0x20215060 as _;
/// Mini Uart Extra Status
/// BCM2835 ARM Peripherals, page 18
const EXTRA_STATUS_REG: *mut u32 = 0x20215064 as _;
/// Mini Uart Baudrate
/// BCM2835 ARM Peripherals, page 19
const BAUDRATE_REG: *mut u32 = 0x20215068 as _;

static READER_WAKER: WakerCell = WAKER_CELL_INIT;

/// The Mini UART peripheral, also referred to as `UART1`.
///
/// # Default Configuration
/// By default, the following configuration is used:
/// - Baud rate: SYSTEM_CLOCK / 8
/// - Bit mode: 7 bits
pub struct MiniUart<RxPin, TxPin> {
    // TODO: is baud_rate & bitmode useless? could just read from the registers.
    baud_rate: u32,
    bit_mode: BitMode,
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
        unsafe { write_volatile(EXTRA_CONTROL_REG, 0) };

        Self {
            baud_rate: 0,
            bit_mode: BitMode::SevenBits,
            lock,
            transmitter_pin: (),
            receiver_pin: (),
        }
    }
}

impl<RxPin, TxPin> MiniUart<RxPin, TxPin> {
    /// Set the baud rate of the Mini UART.
    ///
    /// # Panics
    ///
    /// Panics if the baud rate is not in the range `476..=31_250_000`.
    pub fn set_baud_rate(&mut self, baud_rate: u32) {
        data_memory_barrier();
        assert!(
            // TODO: This should depend on the clock speed.
            (476..=31_250_000).contains(&baud_rate),
            "baud rate not in the range 476..=31_250_000"
        );
        let baud_rate_reg = (CLOCK_SPEED / (8 * baud_rate)) - 1;
        // Safety: Valid address used, data memory barrier used.
        //
        // The input baud rate is in the range 476..=31_250_000, therefore baud_rate_reg is a valid
        // u16 value.
        unsafe {
            write_volatile(BAUDRATE_REG, baud_rate_reg);
        }
        self.baud_rate = baud_rate;
    }

    /// Set the bit mode of the Mini UART.
    pub fn set_bit_mode(&mut self, bit_mode: BitMode) {
        // Safety: Valid address used, data memory barrier used.
        unsafe {
            write_volatile(LINE_CONTROL_REG, bit_mode as u32);
        }
        self.bit_mode = bit_mode;
    }

    /// Get the baud rate of the Mini UART.
    pub fn baud_rate(&self) -> u32 {
        self.baud_rate
    }

    /// Get the bit mode of the Mini UART.
    pub fn bit_mode(&self) -> BitMode {
        self.bit_mode
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
            write_volatile(EXTRA_CONTROL_REG, ((TxPin::ENABLED as u32) << 1) | 1);
            write_volatile(INTERRUPT_ID_REG, 0b10);
        }

        MiniUart {
            baud_rate: self.baud_rate,
            bit_mode: self.bit_mode,
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
            bit_mode: rx_enabled.bit_mode,
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
            write_volatile(EXTRA_CONTROL_REG, 0b10 | RxPin::ENABLED as u32);
            write_volatile(INTERRUPT_ID_REG, 0b100);
        }
        MiniUart {
            baud_rate: self.baud_rate,
            bit_mode: self.bit_mode,
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
            bit_mode: tx_enabled.bit_mode,
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
    /// Disable the Mini UART receiver.
    ///
    /// Drops the receiver pin.
    pub fn disable_receiver(self) -> MiniUart<(), TxPin> {
        data_memory_barrier();
        // Safety: Only addresses defined in the BCM2835 manual are accessed.
        unsafe {
            write_volatile(EXTRA_CONTROL_REG, (TxPin::ENABLED as u32) << 1);
        }
        MiniUart {
            baud_rate: self.baud_rate,
            bit_mode: self.bit_mode,
            lock: self.lock,
            transmitter_pin: self.transmitter_pin,
            receiver_pin: (),
        }
    }
}

impl<RxPin, TxPin> eio::ErrorType for MiniUart<RxPin, TxPin> 
where
    RxPin: MiniUartRxPin<Enabled = True>,
    TxPin: MiniUartTxPin,
{
    type Error = ReadError;
}

impl<RxPin, TxPin> hal_nb::serial::ErrorType for MiniUart<RxPin, TxPin> 
where
    RxPin: MiniUartRxPin<Enabled = True>,
    TxPin: MiniUartTxPin,
{
    type Error = ReadError;
}


impl<RxPin, TxPin> eio::Read for MiniUart<RxPin, TxPin>
where
    RxPin: MiniUartRxPin<Enabled = True>,
    TxPin: MiniUartTxPin,
{
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        data_memory_barrier();
        for (count, byte) in buf.iter_mut().enumerate() {
            loop {
                // Safety: Address is valid, data memory barrier used.
                let status_reg = unsafe { read_volatile(LINE_STATUS_REG) };
                if status_reg & 0b10 != 0 {
                    return Err(ReadError::Overrun);
                } else if status_reg & 1 != 0 {
                    // Safety: As above.
                    *byte = unsafe { read_volatile(IO_REG) as u8 };
                    break;
                } else if count != 0 {
                    return Ok(count);
                }
            }
        }
        Ok(buf.len())
    }

    fn read_exact(
        &mut self,
        buf: &mut [u8],
    ) -> Result<(), embedded_io::ReadExactError<Self::Error>> {
        data_memory_barrier();
        for byte in buf {
            loop {
                // Safety: Address valid, data memory barrier used.
                let status_reg = unsafe { read_volatile(LINE_STATUS_REG) };
                if status_reg & 0b10 != 0 {
                    return Err(ReadError::Overrun.into());
                } else if status_reg & 1 != 0 {
                    break;
                }
            }
            // Safety: As above.
            *byte = unsafe { read_volatile(IO_REG) as u8 };
        }
        Ok(())
    }
}

impl<RxPin, TxPin> eio::ReadReady for MiniUart<RxPin, TxPin>
where
    RxPin: MiniUartRxPin<Enabled = True>,
    TxPin: MiniUartTxPin,
{
    fn read_ready(&mut self) -> Result<bool, Self::Error> {
        data_memory_barrier();
        // Safety: Address is valid, data memory barrier used.
        Ok(unsafe { EXTRA_STATUS_REG.read_volatile() & 1 != 0 })
    }
}

impl<RxPin, TxPin> hal_nb::serial::Read for MiniUart<RxPin, TxPin>
where
    RxPin: MiniUartRxPin<Enabled = True>,
    TxPin: MiniUartTxPin,
{
    fn read(&mut self) -> hal_nb::nb::Result<u8, Self::Error> {
        data_memory_barrier();
        // Safety: Address is valid, data memory barrier used.
        let status_reg = unsafe { read_volatile(LINE_STATUS_REG) };
        if status_reg & 0b10 != 0 {
            return Err(hal_nb::nb::Error::Other(ReadError::Overrun));
        } else if status_reg & 1 != 0 {
            // Safety: As above.
            return Ok(unsafe { read_volatile(IO_REG) as u8 });
        }
        Err(hal_nb::nb::Error::WouldBlock)
    }
}
impl<RxPin, TxPin> eio_async::Read for MiniUart<RxPin, TxPin>
where
    RxPin: MiniUartRxPin<Enabled = True>,
    TxPin: MiniUartTxPin,
{
    /// Because of the way the Mini UART works, this function will almost always read only one
    /// byte. It is more efficient to use the `read_exact` function instead.
    fn read(&mut self, buf: &mut [u8]) -> impl Future<Output = Result<usize, Self::Error>> {
        ReadFut { buf }
    }

    fn read_exact(
        &mut self,
        buf: &mut [u8],
    ) -> impl Future<Output = Result<(), ReadExactError<Self::Error>>> {
        ReadExactFut {
            buf_iter: buf.iter_mut(),
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
                while read_volatile(LINE_STATUS_REG) & 0x20 == 0 {}
                write_volatile(IO_REG, byte as u32);
            }
        }
    }

    /// Disable the Mini UART transmitter.
    ///
    /// Drops the transmitter pin if it was used.
    pub fn disable_transmitter(self) -> MiniUart<RxPin, ()> {
        // Safety: Only addresses defined in the BCM2835 manual are accessed, and bits are set
        //  appropriately.
        unsafe {
            write_volatile(EXTRA_CONTROL_REG, RxPin::ENABLED as u32);
        }
        MiniUart {
            baud_rate: self.baud_rate,
            bit_mode: self.bit_mode,
            lock: self.lock,
            transmitter_pin: (),
            receiver_pin: self.receiver_pin,
        }
    }
}

impl<RxPin, TxPin> eio::ErrorType for MiniUart<RxPin, TxPin>
where
    RxPin: MiniUartRxPin,
    TxPin: MiniUartTxPin<Enabled = True>,
{
    type Error = Infallible;
}

impl<RxPin, TxPin> eio::Write for MiniUart<RxPin, TxPin>
where
    RxPin: MiniUartRxPin,
    TxPin: MiniUartTxPin<Enabled = True>,
{
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        let mut sent = 0;
        data_memory_barrier();
        for byte in bytes {
            // Safety: Only addresses defined in the BCM2835 manual are accessed.
            //  Memory barriers are used according to the BCM2835 manual section 1.3.
            unsafe {
                if read_volatile(LINE_STATUS_REG) & 0x20 == 0 {
                    break;
                }
                write_volatile(IO_REG, byte as u32);
                sent += 1;
            }
        }
        Ok(sent)
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        todo!()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum BitMode {
    #[default]
    SevenBits = 0,
    EightBits = 3,
}

// This exists because you can't destruct structs that `impl Drop`
struct MiniUartLock;

impl MiniUartLock {
    fn get() -> Option<Self> {
        data_memory_barrier();

        critical_section::with(|_| {
            // Safety: Address is valid, and a memory barrier is used. A new `Uart` instance is not
            // created if the peripheral is already in use, and a critical section ensures that
            // two threads do not race to acquire the lock.
            unsafe {
                let enable_state = read_volatile(AUX_ENABLES);
                if enable_state & 1 != 0 {
                    return None;
                }
                write_volatile(AUX_ENABLES, enable_state | 1);
            }
            Some(Self)
        })
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReadError {
    Overrun,
}

impl eio::Error for ReadError {
    fn kind(&self) -> embedded_io::ErrorKind {
        eio::ErrorKind::Other
    }
}

impl hal_nb::serial::Error for ReadError {
    fn kind(&self) -> embedded_hal_nb::serial::ErrorKind {
        hal_nb::serial::ErrorKind::Overrun
    }
}

pub struct ReadFut<'a> {
    buf: &'a mut [u8],
}

impl Future for ReadFut<'_> {
    type Output = Result<usize, ReadError>;

    fn poll(
        mut self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context,
    ) -> Poll<Self::Output> {
        data_memory_barrier();
        for (count, byte) in self.buf.iter_mut().enumerate() {
            // Safety: Address is valid, data memory barrier used.
            let status_reg = unsafe { read_volatile(LINE_STATUS_REG) };
            if status_reg & 0b10 != 0 {
                return Poll::Ready(Err(ReadError::Overrun));
            }

            if status_reg & 1 != 0 {
                // Safety: As above.
                *byte = unsafe { read_volatile(IO_REG) as u8 };
            } else if count != 0 {
                return Poll::Ready(Ok(count));
            } else {

                critical_section::with(|cs| {
                    set_waker(&READER_WAKER, cx.waker(), cs);
                    // Safety: Address is valid, data memory barrier used.
                    let mut reg = unsafe { INTERRUPT_ENABLE_REG.read_volatile() };
                    // We use 1101 because the errata says that bit 2 and 3 should be set to 1.
                    reg |= 0b1101;
                    // Safety: As above.
                    unsafe { INTERRUPT_ENABLE_REG.write_volatile(reg) };
                });
                return Poll::Pending;
            }
        }
        Poll::Ready(Ok(self.buf.len()))
    }
}

pub struct ReadExactFut<'a> {
    buf_iter: IterMut<'a, u8>,
}

impl Future for ReadExactFut<'_> {
    type Output = Result<(), ReadExactError<ReadError>>;

    fn poll(
        mut self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> Poll<Self::Output> {
        data_memory_barrier();
        for byte in self.buf_iter.by_ref() {
            // Safety: Address is valid, data memory barrier used.
            let status_reg = unsafe { read_volatile(LINE_STATUS_REG) };
            if status_reg & 0b10 != 0 {
                return Poll::Ready(Err(ReadExactError::Other(ReadError::Overrun)));
            }

            if status_reg & 1 == 0 {
                critical_section::with(|cs| {
                    set_waker(&READER_WAKER, cx.waker(), cs);
                    // Safety: As above.
                    let mut reg = unsafe { INTERRUPT_ENABLE_REG.read_volatile() };
                    // We use 1101 because the errata says that bit 2 and 3 should be set to 1.
                    reg |= 0b1101;
                    // Safety: As above.
                    unsafe { INTERRUPT_ENABLE_REG.write_volatile(reg) };
                });
                return Poll::Pending;
            }
            // Safety: As above.
            *byte = unsafe { read_volatile(IO_REG) as u8 };
        }
        Poll::Ready(Ok(()))
    }
}

// Clear interrupts and wake the reader/writer.
pub(super) fn interrupt_handler() {
    data_memory_barrier();
    // Safety: Address is valid, data memory barrier used.
    let interrupt_id = unsafe { read_volatile(INTERRUPT_ID_REG) };
    match (interrupt_id >> 1) & 0b11 {
        0b00 => {}
        0b10 => {
            critical_section::with(|cs| {
                // Safety: As above.
                let mut reg = unsafe { INTERRUPT_ENABLE_REG.read_volatile() };
                reg &= !0b01;
                // Safety: As above.
                unsafe { INTERRUPT_ENABLE_REG.write_volatile(reg) };

                if let Some(waker) = READER_WAKER.borrow(cs).take() {
                    waker.wake()
                }
            });
        }
        0b01 => {
            todo!()
        }
        _ => unreachable!(),
    }
}
