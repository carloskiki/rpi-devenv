pub const SPI1: *mut u32 = 0x20215080 as _;
pub const SPI2: *mut u32 = 0x202150C0 as _;

// BCM2835 manual Page 22
pub const CONTROL0: usize = 0x00;
// BCM2835 manual Page 24
pub const CONTROL1: usize = 0x04;
// BCM2835 manual Page 25
pub const STATUS: usize = 0x08;
// BCM2835 manual Page 26
pub const PEEK: usize = 0x0C;
// BCM2835 manual Page 26
pub const IO: usize = 0x20;
// BCM2835 manual Page 27
pub const TXHOLD: usize = 0x30;
