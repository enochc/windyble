
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::thread::sleep;
use std::time::Duration;

use async_std::sync::Arc;


#[cfg(target_arch = "arm")]
use rppal::gpio::{Gpio, InputPin, OutputPin};

#[cfg(not(target_arch = "arm"))]
use crate::mock_gpio::{Gpio, InputPin, OutputPin};

#[allow(unused_imports)]
use log::{info, warn, debug};
use crate::{PinDir, GpioConfig};

#[derive(Clone)]
pub struct Motor {
    gpio_config:GpioConfig,
    running: Arc<AtomicBool>,
    step_duration: Arc<AtomicU64>,
    is_test: bool,
    gpio:Gpio,
}

// impl Clone for Motor {
//     fn clone(&self) -> Self {
//         return Motor {
//             step_pin: self.step_pin.clone(),
//             dir_pin: self.dir_pin.clone(),
//             direction: self.direction.clone(),
//             is_turning: self.is_turning.clone(),
//             step_duration: AtomicU32::new(self.is_turning.load(Ordering::SeqCst)),
//         };
//     }
// }

const SPEED_MIN: u64 = 300;
const SPEED_MAX: u64 = 1_000;
pub const DEFAULT_DURATION: u64 = 400;


impl Motor {
    /*
       Speed value is a percentage 0 to
       returns microseconds, 1,000,000 in a sec
    */
    pub fn set_speed(&self, val: u64) {
        let speed = (((SPEED_MAX - SPEED_MIN) / 100) * val) + SPEED_MIN;
        info!("set speed {}, {}", val, speed);
        self.step_duration.store(speed, Ordering::SeqCst);
    }

    fn get_input(&self, num:u8, reset:bool) ->InputPin {
        let mut pin = self.gpio.get(num).unwrap().into_input();
        pin.set_reset_on_drop(reset);
        return pin;
    }
    fn get_output(&self, num:u8, reset:bool) ->OutputPin {
        let mut pin = self.gpio.get(num).unwrap().into_output();
        pin.set_reset_on_drop(reset);
        return pin;
    }


    pub fn new(gpio_config:GpioConfig, is_test: bool) -> Motor {

        #[cfg(target_arch = "arm")]
        let gpio = Gpio::new().unwrap();

        #[cfg(not(target_arch = "arm"))]
            let gpio = Gpio{};

        return Motor {
            gpio_config,
            running: Arc::new(AtomicBool::new(false)),
            step_duration: Arc::new(AtomicU64::new(u64::from(SPEED_MAX - SPEED_MIN / 2))),
            is_test,
            gpio,
        };
    }

    // TODO this only sets to lowest value... I think
    // set potentiometer
    /*
    p1	p2	Current Limit  is Z high? no, it's an input
    Z	Z	0.5 A
    Low	Z	1 A
    Z	Low	1.5 A
    Low	Low	2 A
     */
    pub fn set_potentiometer(&self, pt_val: &i64) {
        match pt_val {
            1 => {
                self.get_output(self.gpio_config.pt1, false).set_low();
                self.get_input(self.gpio_config.pt2, false);
            },
            2 => {
                self.get_input(self.gpio_config.pt1, false);
                self.get_output(self.gpio_config.pt2, false).set_low();
            },
            3 => {
                self.get_output(self.gpio_config.pt1, false).set_low();
                self.get_output(self.gpio_config.pt2, false).set_low();
            },
            _ => {
                self.get_input(self.gpio_config.pt1, false);
                self.get_input(self.gpio_config.pt2, false);
            }
        }
    }


    fn power_motor(&self, on: bool) {
        debug!("switching motor ({:?}) {}", self.gpio_config.power_relay_pin, if on { "low" } else { "high" });
        let mut pin = self.get_output(self.gpio_config.power_relay_pin, false);
        pin.set_reset_on_drop(false);
        if on {
            pin.set_low();
        } else{
            pin.set_high();
        }
    }

    pub fn is_running(&self)->bool{
        return self.running.load(Ordering::SeqCst);
    }

    pub fn turn(&self, dir: u8) -> bool {
        if self.is_running() {
            info!("Already turning!");
            return false;
        }

        self.power_motor(true);

        self.set_direction(dir);
        let clone = self.clone();
        let speed = if self.is_test {1_000_000} else {self.step_duration.load(Ordering::SeqCst)};
        self.running.store(true, Ordering::SeqCst);
        let run_clone = self.running.clone();
        thread::spawn(move || {
            // println!("thats right {}", self.is_test);
            let mut step_pin = clone.gpio.get(clone.gpio_config.step).expect("Failed to unwrap step pin").into_output();
            while run_clone.load(Ordering::SeqCst) {
                step_pin.set_high();
                sleep(Duration::from_micros(speed));
                step_pin.set_low();
                sleep(Duration::from_micros(speed));
            }
            step_pin.set_low();
            info!("Motor Done turning");
        });
        return true;
    }

    pub fn stop(&self) {
        info!("....... STOP");
        self.running.store(false, Ordering::SeqCst);
        self.power_motor(false);
    }


    pub fn set_direction(&self, dir: u8) {
        info!("SET DIRECTION {:?}", dir);
        // self.dir_pin.set_value(dir.as_u8()).expect("Failed to set direction");
        match dir {
            PinDir::COUNTER_CLOCKWISE => {
                debug!("<<< set dir low");
                // self.gpio.get(self.gpio_config.dir).unwrap().into_output().set_low();
                self.get_output(self.gpio_config.dir, false).set_low();
            }
            _ => {
                debug!("<<< set dir high");
                // self.gpio.get(self.gpio_config.dir).unwrap().into_output().set_high();
                self.get_output(self.gpio_config.dir, false).set_high();
            }

        }

    }

    pub fn init(&self, init_pt:&i64) {
        // self.dir_pin.export().expect("Failed to export DIR pin");
        // self.step_pin.export().expect("Failed to export STEP pin");
        // self.power_pin.export().expect("Failed to export PWR pin");

        // self.pt_pin_1.export().expect("Failed to export pt1");
        // self.pt_pin_2.export().expect("Failed to export pt2");

        // Sleep a moment to allow the pin privileges to update
        // sleep(Duration::from_millis(100));

        // self.step_pin.set_direction(Direction::Low).expect("Failed to set direction on set pin");
        // self.dir_pin.set_direction(Direction::Low).expect("Failed to set direction on direction pin");
        // PT pins default to input mode
        self.set_potentiometer(init_pt);
        // self.power_pin.set_direction(Direction::Out).expect("Failed to set direction on Power pin");
        self.power_motor(false);
    }


    pub fn done(&self) {
        // self.dir_pin.unexport().expect("Failed to un un export DIR pin");
        // self.step_pin.unexport().expect("Failed to un un export STEP pin");
        // dont une-export the power pin or the 12 volt relay will close,
        // best to just leave this on for now
        //self.power_pin.unexport().expect("Failed to un un export PWR pin");
        // self.pt_pin_1.unexport().expect("Failed to un export pt1");
        // self.pt_pin_2.unexport().expect("Failed to un export pt2");
        info!("En-exported pins for motor");
    }

}