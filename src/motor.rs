use std::thread::sleep;
use std::time::Duration;
use crate::{Dir};
use crate::my_pin::MyPin;
use sysfs_gpio::Direction;
use std::sync::atomic::{AtomicBool, Ordering, AtomicU64};
use std::thread;
use async_std::sync::Arc;

// use async_std::task::JoinHandle;

#[derive(Clone)]
pub struct Motor {
    step_pin: MyPin,
    dir_pin: MyPin,
    power_pin: MyPin,
    turn_delay: Duration,
    // todo I can read the pin value for direction, I don't need this property
    direction: u8,
    is_turning: bool,
    step_duration: Arc<AtomicU64>,
}

// impl Clone for Motor {
//     fn clone(&self) -> Self {
//         return Motor {
//             step_pin: self.step_pin.clone(),
//             dir_pin: self.dir_pin.clone(),
//             turn_delay: self.turn_delay.clone(),
//             direction: self.direction.clone(),
//             is_turning: self.is_turning.clone(),
//             step_duration: AtomicU32::new(self.is_turning.load(Ordering::SeqCst)),
//         };
//     }
// }

const SPEED_MIN:u64 = 350;
const SPEED_MAX:u64 = 2_000;



impl Motor {
    /*
       Speed value is a percentage 0 to
       returns microseconds, 1,000,000 in a sec
    */
    pub fn set_speed(& self, val:u64) {

        let speed = (((SPEED_MAX - SPEED_MIN)/ 100) * val) + SPEED_MIN;
        println!("<<<<<< set speed {}, {}", val, speed);
        self.step_duration.store(speed, Ordering::SeqCst);
    }

    #[allow(dead_code)]
    fn is_on(&self) -> bool {
        return self.power_pin.get_value().unwrap() == 1;
    }
    pub fn new(step_pin: MyPin, dir_pin: MyPin, power_pin: MyPin, is_test:bool) -> Motor {
        let duration = if is_test {
            Duration::from_secs(1)
        }else{
            Duration::from_micros(1_000)
        };
        return Motor {
            step_pin,
            dir_pin,
            power_pin,
            turn_delay: duration,
            // turn_delay: Duration::from_secs(1),
            direction: Dir::CLOCKWISE,
            is_turning: false,
            step_duration: Arc::new(AtomicU64::new(u64::from(SPEED_MAX - SPEED_MIN / 2)))
        };
    }

    fn power_motor(&self, on:bool) {
        let val = if on {1} else {0};
        self.power_pin.set_value(val).expect("Failed to change motor power");
    }


    pub fn turn(&mut self, dir: u8)->Option<Arc<AtomicBool>> {
        if self.is_turning {
            println!("Already turning!");
            return None::<Arc<AtomicBool>>;
        }
        self.power_motor(true);
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();
        self.set_direction(dir);
        self.is_turning = true;
        let clone = self.clone();
        println!("<<<< .... TURN AWAY: {:?} .... >>>>", clone.step_duration);
        let speed = self.step_duration.load(Ordering::SeqCst);
        thread::spawn(move ||{
            while running_clone.load(Ordering::SeqCst) {
                clone.step_pin.set_value(1).unwrap();
                sleep(Duration::from_micros(speed));
                clone.step_pin.set_value(0).unwrap();
                sleep(Duration::from_micros(speed));
            }
            println!("DONE");
        });
        return Some(running);
    }

    pub fn stop(&mut self) {
        self.is_turning = false;
        self.step_pin.set_value(0).unwrap();
        self.power_motor(false);
    }


    pub fn set_direction(&mut self, dir: u8) {
        self.direction = dir;
        self.dir_pin.set_value(dir).expect("Failed to set direction");
    }

    pub fn init(&self) {
        self.dir_pin.export().expect("Failed to export DIR pin");
        self.step_pin.export().expect("Failed to export STEP pin");
        self.power_pin.export().expect("Failed to export PWR pin");
        // Sleep a moment to allow the pin privileges to update
        sleep(Duration::from_millis(80));

        self.step_pin.set_direction(Direction::Out).expect("Failed to set direction on set pin");
        self.dir_pin.set_direction(Direction::Out).expect("Failed to set direction on direction pin");
        self.power_pin.set_direction(Direction::Out).expect("Failed to set direction on power pin");
    }
    pub fn done(&self) {
        self.dir_pin.unexport().expect("Failed to un un export DIR pin");
        self.step_pin.unexport().expect("Failed to un un export STEP pin");
        self.power_pin.unexport().expect("Failed to un un export PWR pin");
    }

    #[allow(dead_code)]
    pub fn turn_once(&self) {
        self.power_motor(true);
        for _x in 0..200 {
            self.step_pin.set_value(1).unwrap();
            sleep(self.turn_delay);
            self.step_pin.set_value(0).unwrap();
            sleep(self.turn_delay);
        }
        self.power_motor(false);
    }
}