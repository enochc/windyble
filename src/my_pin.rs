use sysfs_gpio::{Direction, Pin, Error};

#[allow(unused_imports)]
use log::{info, warn, debug};

#[cfg(target_arch = "arm")]
use rppal::gpio::{Gpio, OutputPin};


#[derive(Clone)]
pub struct MyPin {
    pub pin: Option<Pin>,
    pub number:u8,
    pub is_test:bool
}

impl MyPin {
    pub fn new(number:u8, is_test:bool) -> MyPin {
        debug!("new MyPin");
        let pin = if !is_test {
            Some(Pin::new(number.into()))
        } else { None };
        pin.unwrap().unexport().expect(&format!("Failed to unexport pin {:?}", number));
        return MyPin {
            pin,
            number,
            is_test
        };
    }

    pub fn get_value(&self)-> sysfs_gpio::Result<u8> {
        // self.pin.unwrap().get_value()
        return match self.pin {
            Some(p) => p.get_value(),
            None => {
                debug!("get value of PIN {:?}", self.number);
                Ok(0)
            }
        };
    }

    pub fn set_value(&self, val: u8) -> Result<(), Error> {
        return match self.pin {
            Some(p) => {
                p.set_value(val)
            },
            None => {
                debug!("Set PIN {:?} = {:?}",self.number, val);
                Ok(())
            }
        };
    }
    pub fn set_direction(&self, val: Direction) -> Result<(), Error> {
        return match self.pin {
            Some(p) => p.set_direction(val),
            None => {
                debug!("Set Direction {:?} = {:?}",self.number, val);
                Ok(())
            }
        };
    }
    pub fn export(&self) -> Result<(), Error> {
        return match self.pin {
            Some(p) => p.export(),
            None => {
                debug!("Export PIN {:?}", self.number);
                Ok(())
            }
        };
    }
    pub fn unexport(&self) -> Result<(), Error> {
        return match self.pin {
            Some(p) => p.unexport(),
            None => {
                debug!("UnExport PIN {:?}",self.number);
                Ok(())
            }
        };
    }
}