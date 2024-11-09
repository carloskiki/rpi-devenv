/// Mini Uart I/O Data
/// BCM2835 ARM Peripherals, page 11
pub const IO_REG: *mut u32 = 0x20215040 as _;
/// Mini Uart Interrupt Enable
/// BCM2835 ARM Peripherals, page 12
pub const INTERRUPT_ENABLE_REG: *mut u32 = 0x20215044 as _;
/// Mini Uart Interrupt Identify
/// BCM2835 ARM Peripherals, page 13
pub const INTERRUPT_ID_REG: *mut u32 = 0x20215048 as _;
/// Mini Uart Line Control
/// BCM2835 ARM Peripherals, page 14
pub const LINE_CONTROL_REG: *mut u32 = 0x2021504C as _;
/// Mini Uart Modem Control
/// BCM2835 ARM Peripherals, page 14
pub const MODEM_CONTROL_REG: *mut u32 = 0x20215050 as _;
/// Mini Uart Line Status
/// BCM2835 ARM Peripherals, page 15
pub const LINE_STATUS_REG: *mut u32 = 0x20215054 as _;
/// Mini Uart Modem Status
/// BCM2835 ARM Peripherals, page 15
pub const MODEM_STATUS_REG: *mut u32 = 0x20215058 as _;
/// Mini Uart Extra Control
/// BCM2835 ARM Peripherals, page 16
pub const EXTRA_CONTROL_REG: *mut u32 = 0x20215060 as _;
/// Mini Uart Extra Status
/// BCM2835 ARM Peripherals, page 18
pub const EXTRA_STATUS_REG: *mut u32 = 0x20215064 as _;
/// Mini Uart Baudrate
/// BCM2835 ARM Peripherals, page 19
pub const BAUDRATE_REG: *mut u32 = 0x20215068 as _;
