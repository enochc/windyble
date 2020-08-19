use std::thread::sleep;
use std::time::Duration;
use crate::{Dir};
use crate::my_pin::MyPin;
use sysfs_gpio::Direction;
use std::sync::atomic::{AtomicBool, Ordering, AtomicU64};
use std::thread;
use async_std::sync::Arc;


#[derive(Clone)]
pub struct Motor {
    step_pin: MyPin,
    dir_pin: MyPin,
    power_pin: MyPin,
    pt_pin_1: MyPin,
    pt_pin_2: MyPin,
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
    pub fn new(step_pin: MyPin, dir_pin: MyPin, power_pin: MyPin, pt_pin_1:MyPin, pt_pin_2:MyPin, is_test:bool) -> Motor {
        let duration = if is_test {
            Duration::from_secs(1)
        }else{
            Duration::from_micros(1_000)
        };
        return Motor {
            step_pin,
            dir_pin,
            power_pin,
            pt_pin_1,
            pt_pin_2,
            turn_delay: duration,
            // turn_delay: Duration::from_secs(1),
            direction: Dir::CLOCKWISE,
            is_turning: false,
            step_duration: Arc::new(AtomicU64::new(u64::from(SPEED_MAX - SPEED_MIN / 2))),

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
    pub fn set_potentiometer(&self, pt_val:i64){
        match pt_val {
            1 => {
                self.pt_pin_1.set_direction(Direction::Low).expect("Failed to set direction on pt pin1");
                self.pt_pin_2.set_direction(Direction::In).expect("Failed to set direction on pt pin2");
            },
            2 => {
                self.pt_pin_1.set_direction(Direction::In).expect("Failed to set direction on pt pin1");
                self.pt_pin_2.set_direction(Direction::Low).expect("Failed to set direction on pt pin2");
            },
            3 => {
                self.pt_pin_1.set_direction(Direction::Low).expect("Failed to set direction on pt pin1");
                self.pt_pin_2.set_direction(Direction::Low).expect("Failed to set direction on pt pin2");
            },
            _ => {
                // Default to .5 A
                self.pt_pin_1.set_direction(Direction::In).expect("Failed to set direction on pt1 pin");
                self.pt_pin_2.set_direction(Direction::In).expect("Failed to set direction on pt2 pin");
            }
        }
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

        self.pt_pin_1.export().expect("Failed to export pt1");
        self.pt_pin_2.export().expect("Failed to export pt2");


        println!("all things exported");
        // Sleep a moment to allow the pin privileges to update
        sleep(Duration::from_millis(80));

        self.step_pin.set_direction(Direction::Low).expect("Failed to set direction on set pin");
        self.dir_pin.set_direction(Direction::Low).expect("Failed to set direction on direction pin");
        self.power_pin.set_direction(Direction::Low).expect("Failed to set direction on power pin");
        // PT pins default to input mode
        self.set_potentiometer(0);

        //TODO revisit this
        // self.set_potentiometer(0);
    }
    pub fn done(&self) {
        self.dir_pin.unexport().expect("Failed to un un export DIR pin");
        self.step_pin.unexport().expect("Failed to un un export STEP pin");
        self.power_pin.unexport().expect("Failed to un un export PWR pin");
        self.pt_pin_1.unexport().expect("Failed to un export pt2");
        self.pt_pin_2.unexport().expect("Failed to un export pt2");
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