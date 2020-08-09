mod motor;
mod my_pin;

use sysfs_gpio::{Direction, Pin, Error};
use std::thread::{sleep};
use std::time::Duration;
use hive::hive::Hive;
use async_std::task;
use async_std::sync::{Arc};
use std::sync::{Condvar, Mutex};
use std::sync::atomic::{Ordering, AtomicU8, AtomicBool};

use futures::channel::mpsc;
use futures::{SinkExt, StreamExt};
use futures::executor::block_on;
use crate::my_pin::MyPin;
use crate::motor::Motor;
use std::borrow::BorrowMut;
use std::thread;


const STEP: u64 = 26;
const DIR: u64 = 19;

struct Dir;

impl Dir {
    const CLOCKWISE: u8 = 1;
    const COUNTER_CLOCKWISE: u8 = 0;
}


fn main() {
// PI 192.168.5.41:3000
    let hive_props = r#"
    listen = "127.0.0.1:3000"
    [Properties]
    moveup = false
    movedown = false
    speed = 1000
    "#;


    let move_up = Arc::new(AtomicBool::new(false));
    let move_down = Arc::new(AtomicBool::new(false));

    let mut pi_hive = Hive::new_from_str("SERVE", hive_props);

    // FOR TESTING ON NOT A PI
    let TEST = true;
    let step_pin = MyPin::new(STEP, TEST);
    let dir_pin = MyPin::new(DIR, TEST);

    let mut motor = Motor::new(step_pin, dir_pin);

    // let move_up_clone = move_up.clone();
    let up_pair = Arc::new((Mutex::new(false), Condvar::new()));
    let up_pair2 = up_pair.clone();
    pi_hive.get_mut_property("moveup").unwrap().on_changed.connect(move |value| {
        println!("<<<< MOVE UP: {:?}", value);
        let (lock, cvar) = &*up_pair2;
        let mut going_up = lock.lock().unwrap();
        let val = value.unwrap().as_bool().unwrap();
        *going_up = val;

        cvar.notify_one();
    });

    pi_hive.get_mut_property("movedown").unwrap().on_changed.connect(move |value| {
        println!("<<<< MOVE DOWN: {:?}", value);
        let val = value.unwrap().as_bool().unwrap();
        move_down.store(val, Ordering::SeqCst);
    });

    let (mut sender, mut receiver) = mpsc::unbounded();

    task::spawn(async move {
        pi_hive.run().await;
    });

    motor.init();

    let mut motor_clone = motor.clone();
    task::spawn(async move {
        let (lock, cvar) = &*up_pair;
        let mut upping = lock.lock().unwrap();

        while !*upping {
            upping = cvar.wait(upping).unwrap();
            let mut running = motor_clone.turn(Dir::CLOCKWISE);
            println!("<< GO UP {:?}", upping);
            while *upping {
                upping = cvar.wait(upping).unwrap();
                println!("<< Stop {:?}", upping);
                running.unwrap().store(false, Ordering::SeqCst);
                motor_clone.stop();
                break;
            }
        }
        sender.send(1);
    });

    let done = block_on(receiver.next());

    // loop {
    //     move_up_new = move_up.load(Ordering::Acquire);
    //     if move_up_current != move_up_new {
    //         move_up_current = move_up_new;
    //         if move_up_current {
    //             println!("<<<< moveup");
    //             motor.turn(Dir::CLOCKWISE);
    //         }  else {
    //             println!("<<<<  stop");
    //             motor.stop()
    //         }
    //     }
    // }


    motor.done();

    println!("Done");
}
