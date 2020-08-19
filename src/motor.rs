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
    turn_delay: Duration,
    // todo I can read the pin value for direction, I don't need this property
    direction: u8,
    is_turning: bool,
    step_duration: Arc<AtomicU64>,
    pt_pin_1: Option<MyPin>,
    pt_pin_2: Option<MyPin>,
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
            step_duration: Arc::new(AtomicU64::new(u64::from(SPEED_MAX - SPEED_MIN / 2))),
            pt_pin_1:None,
            pt_pin_2:None,
        };
    }

    pub fn set_pt_pins(&mut self, pt1:MyPin, pt2:MyPin){
        self.pt_pin_1 = Some(pt1);
        self.pt_pin_2 = Some(pt2);
    }

    // TODO this only sets to lowest value... I think
    // set potentiometer
    /*
    p1	p2	Current Limit  is Z high?
    Z	Z	0.5 A
    Low	Z	1 A
    Z	Low	1.5 A
    Low	Low	2 A
     */
    pub fn setPotentiometer(&self, pt_val:i8){
        match &self.pt_pin_1 {
            Some(p) => {
                p.set_value(1).expect("Failed to set Pt1 pin value");
                // if pt1 exists, so does pt1
                self.pt_pin_2.as_ref().unwrap().set_value(1).expect("Failed to set Pt2 pin value");
                println!("Set pt pins High (for low amperage)");
            },
            _ => {println!("No potentiometer pins")}
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
        match &self.pt_pin_1 {
            Some(p) => {
                p.export().expect("Failed to export pt1");
                self.pt_pin_2.as_ref().unwrap().export().expect("Failed to export pt2")
            },
            _ => println!("No pt pins")

        }
        // Sleep a moment to allow the pin privileges to update
        sleep(Duration::from_millis(80));

        self.step_pin.set_direction(Direction::Out).expect("Failed to set direction on set pin");
        self.dir_pin.set_direction(Direction::Out).expect("Failed to set direction on direction pin");
        self.power_pin.set_direction(Direction::Out).expect("Failed to set direction on power pin");

        //TODO revisit this
        self.setPotentiometer(0);
    }
    pub fn done(&self) {
        self.dir_pin.unexport().expect("Failed to un un export DIR pin");
        self.step_pin.unexport().expect("Failed to un un export STEP pin");
        self.power_pin.unexport().expect("Failed to un un export PWR pin");
        match &self.pt_pin_1 {
            Some(p) => {
                p.unexport().expect("Failed to un export pt1");
                self.pt_pin_2.as_ref().unwrap().unexport().expect("Failed to un export pt2");
            },
            _ => println!("No pt pins")
        }
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