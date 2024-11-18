use crate::{impl_sealed, Sealed};

#[allow(private_bounds)]
pub trait Mode: Sealed {
    const CONFIG_MASK: u32;
}

pub struct Variable;

impl_sealed!(Variable);

impl Mode for Variable {
    // This turns on both the Variable length and variable CS modes in the CNTL0 register
    const CONFIG_MASK: u32 = 0b11 << 14;
}
