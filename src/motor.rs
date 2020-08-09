use std::thread::sleep;
use std::time::Duration;
use crate::Dir;
use crate::my_pin::MyPin;
use sysfs_gpio::Direction;
use async_std::task;
use futures::channel::mpsc::UnboundedReceiver;
use futures::channel::mpsc;
use std::sync::atomic::{AtomicBool, Ordering};

// #[derive(Clone)]
pub struct Motor {
    step_pin: MyPin,
    dir_pin: MyPin,
    turn_delay: Duration,
    direction: u8,
    // true or false for clockwise, counterclockwise
    is_turning: AtomicBool,
}

impl Clone for Motor {
    fn clone(&self) -> Self {
        return Motor {
            step_pin: self.step_pin.clone(),
            dir_pin: self.dir_pin.clone(),
            turn_delay: self.turn_delay.clone(),
            direction: self.direction.clone(),
            is_turning: AtomicBool::new(self.is_turning.load(Ordering::SeqCst)),
        };
    }
}

impl Motor {
    pub fn new(step_pin: MyPin, dir_pin: MyPin) -> Motor {
        return Motor {
            step_pin,
            dir_pin,
            turn_delay: Duration::from_micros(1000),
            direction: Dir::CLOCKWISE,
            is_turning: AtomicBool::new(false),
        };
    }


    pub fn turn(&mut self, dir: u8) {
        if self.is_turning.load(Ordering::SeqCst) {
            println!("Already turning!");
            return;
        }
        self.set_direction(dir);
        // self.is_turning.store(true,Ordering::SeqCst);
        let clone = self.clone();
        task::spawn(async move {
            while clone.is_turning.load(Ordering::SeqCst) {
                clone.step_pin.set_value(1).unwrap();
                sleep(clone.turn_delay);
                clone.step_pin.set_value(0).unwrap();
                sleep(clone.turn_delay);
            }
        });
    }

    pub fn stop(&mut self) {
        self.is_turning.store(false, Ordering::SeqCst);
        self.step_pin.set_value(0).unwrap();
    }


    pub fn set_direction(&mut self, dir: u8) {
        self.direction = dir;
        self.dir_pin.set_value(dir).expect("Failed to set direction");
    }

    pub fn init(&self) {
        self.dir_pin.export().expect("Failed to export DIR pin");
        self.step_pin.export().expect("Failed to export DIR pin");
        // Sleep a moment to allow the pin privileges to update
        sleep(Duration::from_millis(80));

        self.step_pin.set_direction(Direction::Out).expect("Failed to set direction on set pin");
        self.dir_pin.set_direction(Direction::Out).expect("Failed to set direction on direction pin");
    }
    pub fn done(&self) {
        self.dir_pin.unexport().expect("Failed to un export DIR pin");
        self.step_pin.unexport().expect("Failed to un export DIR pin");
    }

    pub fn turn_once(&self) {
        for _x in 0..200 {
            self.step_pin.set_value(1).unwrap();
            sleep(self.turn_delay);
            self.step_pin.set_value(0).unwrap();
            sleep(self.turn_delay);
        }
    }
}