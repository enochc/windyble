extern crate sysfs_gpio;

use sysfs_gpio::{Direction, Pin};
use std::thread::{sleep};
use std::time::Duration;
use hive::hive::Hive;
use async_std::task;
use async_std::sync::Arc;
use std::sync::atomic::{Ordering, AtomicU8, AtomicBool};
// use std::option::NoneError;

// use std::error::Error;
use futures::channel::mpsc;
use futures::{SinkExt, StreamExt};


const STEP: u64 = 26;
const DIR: u64 = 19;

// pub type Result<T> = ::std::result::Result<T, dyn std::error::Error>;
// type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

struct Dir;
impl Dir{
    const CLOCKWISE:u8=1;
    const COUNTER_CLOCKWISE:u8=0;
}

#[derive(Clone)]
pub struct Motor {
    step_pin: Pin,
    dir_pin: Pin,
    turn_delay: Duration,
    direction: u8, // true or false for clockwise, counterclockwise
    is_turning: bool,
}

impl Motor {
    fn new(step_pin: Pin, dir_pin: Pin) -> Motor {
        return Motor {
            step_pin,
            dir_pin,
            turn_delay: Duration::from_micros(1000),
            direction: Dir::CLOCKWISE,
            is_turning: false,
        };
    }

    fn turn(&mut self, dir:u8){
        self.set_direction(dir);
        self.is_turning = true;
        while self.is_turning {
            self.step_pin.set_value(1).unwrap();
            sleep(self.turn_delay);
            self.step_pin.set_value(0).unwrap();
            sleep(self.turn_delay);
        }
    }

    fn stop(&mut self){
        self.is_turning = false;
        self.step_pin.set_value(0).unwrap();
    }


    fn set_direction(&mut self, dir:u8){
        self.direction = dir;
        self.dir_pin.set_value(dir).expect("Failed to set direction");
    }

    fn init(&self ){
        self.dir_pin.export().expect("Failed to export DIR pin");
        self.step_pin.export().expect("Failed to export DIR pin");
        // Sleep a moment to allow the pin privileges to update
        sleep(Duration::from_millis(80));

        self.step_pin.set_direction(Direction::Out).expect("Failed to set direction on set pin");
        self.dir_pin.set_direction(Direction::Out).expect("Failed to set direction on direction pin");
    }
    fn done(&self){
        self.dir_pin.unexport().expect("Failed to un export DIR pin");
        self.step_pin.unexport().expect("Failed to un export DIR pin");
    }

    fn turn_once(&self){
        for _x in 0..200 {
            self.step_pin.set_value(1).unwrap();
            sleep(self.turn_delay);
            self.step_pin.set_value(0).unwrap();
            sleep(self.turn_delay);
        }
    }


}


fn main() {

    let hive_props = r#"
    listen = "192.168.5.41:3000"
    [Properties]
    moveup = false
    movedown = false
    speed = 1000
    "#;

    let move_up = Arc::new(AtomicBool::new(false));
    let move_down = Arc::new(AtomicBool::new(false));

    let mut pi_hive = Hive::new_from_str("SERVE", hive_props);

    let step_pin = Pin::new(STEP);
    let dir_pin = Pin::new(DIR);
    let mut motor = Motor::new(step_pin, dir_pin);

    pi_hive.get_mut_property("moveup").unwrap().on_changed.connect( move|value|{
        println!("<<<< MOVE UP: {:?}", value);
        let val = value.unwrap().as_bool().unwrap();
        move_up.store(val, Ordering::SeqCst);
    });
    pi_hive.get_mut_property("move_down").unwrap().on_changed.connect(move |value|{
        println!("<<<< MOVE DOWN: {:?}", value);
        let val = value.unwrap().as_bool().unwrap();
        move_down.store(val, Ordering::SeqCst);
    });

    task::spawn(async move {
        pi_hive.run().await.expect("Have failed");
    });

    let (mut sender, mut receiver) = mpsc::unbounded();
    motor.init();
    let mut min = motor.clone();
    task::spawn(async move{
        // to motor work in here
        min.turn(Dir::CLOCKWISE);
        sleep(Duration::from_millis(500));
        min.turn(Dir::COUNTER_CLOCKWISE);


        sender.send(true).await.expect("Failed to send end signal");
    });
    let done = receiver.next();
    motor.stop();
    println!("DONE!!");
    motor.done();


    // let light_value = Arc::new(AtomicU8::new(0));
    //
    // let hive_change_light_value = light_value.clone();
    // //TODO make onchanged.connect an FnMut so I can pass in a channel that sends a value
    //
    //
    //

    //
    // let led_loop_value = light_value.clone();
    // my_led.set_direction(Direction::Out).unwrap();
    // step_pin.with_exported(move|| {
    //     let mut last:u8 = 0;
    //
    //     loop {
    //         let v = led_loop_value.load(Ordering::SeqCst);
    //         if v != last {
    //             println!("Setting value: {}", v);
    //             my_led.set_value(v).unwrap();
    //             last = v;
    //         }
    //         // sleep a moment just to slow things down
    //         sleep(Duration::from_millis(200));
    //     }
    //     // Ok(())
    // }).unwrap();

    println!("Done");
}
