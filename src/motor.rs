use std::thread::sleep;
use std::time::Duration;
use crate::Dir;
use crate::my_pin::MyPin;
use sysfs_gpio::Direction;
use async_std::task;
// use runtime::task;
use futures::channel::mpsc::UnboundedReceiver;
use futures::channel::mpsc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use async_std::sync::Arc;
// use async_std::task::JoinHandle;

#[derive(Clone)]
pub struct Motor {
    step_pin: MyPin,
    dir_pin: MyPin,
    turn_delay: Duration,
    direction: u8,
    // true or false for clockwise, counterclockwise
    is_turning: bool,
}

// impl Clone for Motor {
//     fn clone(&self) -> Self {
//         return Motor {
//             step_pin: self.step_pin.clone(),
//             dir_pin: self.dir_pin.clone(),
//             turn_delay: self.turn_delay.clone(),
//             direction: self.direction.clone(),
//             is_turning: AtomicBool::new(self.is_turning.load(Ordering::SeqCst)),
//         };
//     }
// }

impl Motor {
    pub fn new(step_pin: MyPin, dir_pin: MyPin) -> Motor {
        return Motor {
            step_pin,
            dir_pin,
            turn_delay: Duration::from_micros(1000),
            direction: Dir::CLOCKWISE,
            is_turning: false,
        };
    }


    pub fn turn(&mut self, dir: u8)->Option<Arc<AtomicBool>> {
        if self.is_turning {
            println!("Already turning!");
            return None::<Arc<AtomicBool>>;
        }
        let mut running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();
        self.set_direction(dir);
        self.is_turning = true;
        let clone = self.clone();
        println!("TURN AWAY");
        thread::spawn(move ||{
            while running_clone.load(Ordering::SeqCst) {
                clone.step_pin.set_value(1).unwrap();
                sleep(clone.turn_delay);
                clone.step_pin.set_value(0).unwrap();
                sleep(clone.turn_delay);
            }
            println!("DONE");
        });
        return Some(running);
    }

    pub fn stop(&mut self) {
        self.is_turning = false;
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