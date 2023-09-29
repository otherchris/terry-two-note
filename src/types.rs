use crate::{pac::I2C0, Gpio16, Gpio17, Pin, I2C};
use rp2040_hal::gpio::{FunctionI2c, PullDown};

pub type SdaPin = rp2040_hal::gpio::Pin<Gpio16, FunctionI2c, PullDown>;
pub type SclPin = rp2040_hal::gpio::Pin<Gpio17, FunctionI2c, PullDown>;
pub type I2CType = I2C<I2C0, (SdaPin, SclPin)>;
