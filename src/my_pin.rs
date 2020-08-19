// extern crate sysfs_gpio;
use sysfs_gpio::{Direction, Pin, Error};

#[derive(Clone)]
pub struct MyPin {
    pin: Option<Pin>,
    number:u64,
    is_test:bool
}

impl MyPin {
    pub fn new(number:u64, is_test:bool) -> MyPin {
        let pin = if !is_test {
            Some(Pin::new(number))
        } else { None };
        return MyPin {
            pin,
            number,
            is_test
        };
    }
    pub fn get_value(&self)-> sysfs_gpio::Result<u8> {
        self.pin.unwrap().get_value()
    }

    pub fn set_value(&self, val: u8) -> Result<(), Error> {
        return match self.pin {
            Some(p) => {
                p.set_value(val)
            },
            None => {
                println!("Set PIN {:?} = {:?}",self.number, val);
                Ok(())
            }
        };
    }
    pub fn set_direction(&self, val: Direction) -> Result<(), Error> {
        return match self.pin {
            Some(p) => p.set_direction(val),
            None => {
                println!("Set Direction {:?} = {:?}",self.number, val);
                Ok(())
            }
        };
    }
    pub fn export(&self) -> Result<(), Error> {
        return match self.pin {
            Some(p) => p.export(),
            None => {
                println!("Export PIN {:?}", self.number);
                Ok(())
            }
        };
    }
    pub fn unexport(&self) -> Result<(), Error> {
        return match self.pin {
            Some(p) => p.unexport(),
            None => {
                println!("UnExport PIN {:?}",self.number);
                Ok(())
            }
        };
    }
}