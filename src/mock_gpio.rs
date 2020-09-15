use std::result;
use sysfs_gpio::Error;


pub type Result<T> = result::Result<T, Error>;

#[allow(dead_code)]
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
#[repr(u8)]
pub enum Level {
    Low = 0,
    High = 1,
}

#[derive(Clone)]
pub struct Gpio{}


pub struct Pin;
pub struct OutputPin;
pub struct InputPin;

impl Gpio {
    pub fn get(&self, _: u8) -> Result<Pin> {
        return Ok(Pin {});
    }

    pub fn new() -> Result<Gpio> {
        return Ok(Gpio {});
    }

}

impl Pin{
    pub fn into_output(&self)->OutputPin {
        return OutputPin{};
    }
    pub fn into_input(&self)->InputPin {
        return InputPin{};
    }
    pub fn into_input_pulldown(&self)->InputPin {
        return InputPin{};
    }


}

impl OutputPin {
    pub fn set_low(&mut self){}
    pub fn set_high(&mut self){}
    pub fn set_reset_on_drop(&mut self, _:bool){}
}

impl InputPin {
    pub fn read(&self) -> Level {
        return Level::High;
    }
    pub fn set_reset_on_drop(&mut self, _:bool){}
}