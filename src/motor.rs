
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::thread::sleep;
use std::time::Duration;

use async_std::sync::Arc;

use crate::my_pin::MyPin;

use sysfs_gpio::{Direction};

#[allow(unused_imports)]
use log::{info, warn, debug};

#[derive(Clone)]
pub struct Motor {
    step_pin: MyPin,
    dir_pin: MyPin,
    power_pin: MyPin,
    pt_pin_1: MyPin,
    pt_pin_2: MyPin,
    running: Arc<AtomicBool>,//::new(AtomicBool::new(true));
    step_duration: Arc<AtomicU64>,
    is_test: bool,
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

const SPEED_MIN: u64 = 350;
const SPEED_MAX: u64 = 1_000;
pub const DEFAULT_DURATION: u64 = 500;


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

    #[allow(dead_code)]
    fn is_on(&self) -> bool {
        return self.power_pin.get_value().unwrap() == 0;
    }

    pub fn new(step_pin: MyPin,
               dir_pin: MyPin,
               power_pin: MyPin,
               pt_pin_1: MyPin,
               pt_pin_2: MyPin,
               is_test: bool) -> Motor
    {

        return Motor {
            step_pin,
            dir_pin,
            power_pin,
            pt_pin_1,
            pt_pin_2,
            running: Arc::new(AtomicBool::new(false)),
            step_duration: Arc::new(AtomicU64::new(u64::from(SPEED_MAX - SPEED_MIN / 2))),
            is_test
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
    pub fn set_potentiometer(&self, pt_val: i64) {
        match pt_val {
            1 => {
                self.pt_pin_1.set_direction(Direction::Low).expect("Failed to set direction on pt pin1");
                self.pt_pin_2.set_direction(Direction::In).expect("Failed to set direction on pt pin2");
            }
            2 => {
                self.pt_pin_1.set_direction(Direction::In).expect("Failed to set direction on pt pin1");
                self.pt_pin_2.set_direction(Direction::Low).expect("Failed to set direction on pt pin2");
            }
            3 => {
                self.pt_pin_1.set_direction(Direction::Low).expect("Failed to set direction on pt pin1");
                self.pt_pin_2.set_direction(Direction::Low).expect("Failed to set direction on pt pin2");
            }
            _ => {
                // Default to .5 A
                self.pt_pin_1.set_direction(Direction::In).expect("Failed to set direction on pt1 pin");
                self.pt_pin_2.set_direction(Direction::In).expect("Failed to set direction on pt2 pin");
            }
        }
    }

    fn power_motor(&self, on: bool) {
        let num = self.power_pin.number;
        info!("switching motor ({:?}) {}", num, if on { "on" } else { "off" });
        let val = if on { 0 } else { 1 };
        self.power_pin.set_value(val).expect("Failed to change motor power");
    }
    pub fn is_running(&self)->bool{
        return self.running.load(Ordering::SeqCst);
    }


    pub fn turn(&mut self, dir: u8) -> bool {
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
            while run_clone.load(Ordering::SeqCst) {
                clone.step_pin.set_value(1).unwrap();
                sleep(Duration::from_micros(speed));
                clone.step_pin.set_value(0).unwrap();
                sleep(Duration::from_micros(speed));
            }
            info!("Motor Done turning");
        });
        return true;
    }

    pub fn stop(&mut self) {
        info!("....... STOP");
        self.running.store(false, Ordering::SeqCst);
        self.step_pin.set_value(0).unwrap();
        self.power_motor(false);
    }


    pub fn set_direction(&self, dir: u8) {
        info!("SET DIRECTION {:?}", dir);
        self.dir_pin.set_value(dir).expect("Failed to set direction");
    }

    pub fn init(&self) {
        self.dir_pin.export().expect("Failed to export DIR pin");
        self.step_pin.export().expect("Failed to export STEP pin");
        self.power_pin.export().expect("Failed to export PWR pin");

        self.pt_pin_1.export().expect("Failed to export pt1");
        self.pt_pin_2.export().expect("Failed to export pt2");

        // Sleep a moment to allow the pin privileges to update
        sleep(Duration::from_millis(100));

        self.step_pin.set_direction(Direction::Low).expect("Failed to set direction on set pin");
        self.dir_pin.set_direction(Direction::Low).expect("Failed to set direction on direction pin");
        // PT pins default to input mode
        self.set_potentiometer(0);
        self.power_pin.set_direction(Direction::Out).expect("Failed to set direction on Power pin");
        self.power_motor(false);
    }


    pub fn done(&self) {
        self.dir_pin.unexport().expect("Failed to un un export DIR pin");
        self.step_pin.unexport().expect("Failed to un un export STEP pin");
        self.power_pin.unexport().expect("Failed to un un export PWR pin");
        self.pt_pin_1.unexport().expect("Failed to un export pt2");
        self.pt_pin_2.unexport().expect("Failed to un export pt2");

    }


    // fn poll(&self, pin_num: u64) -> sysfs_gpio::Result<()> {
    //     let input = Pin::new(pin_num);
    //     input.with_exported(|| {
    //         input.set_direction(Direction::In)?;
    //         let mut prev_val: u8 = 255;
    //         loop {
    //             let val = input.get_value()?;
    //             if val != prev_val {
    //                 println!("Pin State: {}", if val == 0 { "Low" } else { "High" });
    //                 prev_val = val;
    //             }
    //             sleep(Duration::from_millis(10));
    //         }
    //     })
    // }
}