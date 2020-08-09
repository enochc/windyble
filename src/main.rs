mod motor;
mod my_pin;

use hive::hive::Hive;
use async_std::task;
use async_std::sync::{Arc};
use std::sync::{Condvar, Mutex};
use std::sync::atomic::{Ordering, AtomicBool, AtomicU8};

use futures::channel::mpsc;
use futures::{SinkExt, StreamExt};
use futures::executor::block_on;
use crate::my_pin::MyPin;
use crate::motor::Motor;


const STEP: u64 = 26;
const DIR: u64 = 19;
// FOR TESTING ON NOT A PI
const TEST: bool = true;

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

    // let move_up = Arc::new(AtomicBool::new(false));
    // let move_down = Arc::new(AtomicBool::new(false));

    let mut pi_hive = Hive::new_from_str("SERVE", hive_props);

    let step_pin = MyPin::new(STEP, TEST);
    let dir_pin = MyPin::new(DIR, TEST);

    let motor = Motor::new(step_pin, dir_pin);

    // let move_up_clone = move_up.clone();
    let up_pair = Arc::new((Mutex::new(false), Condvar::new()));
    let up_pair2 = up_pair.clone();
    let up_pair3 = up_pair.clone();
    let current_dir = Arc::new(AtomicU8::new(0));
    let current_dir_clone = current_dir.clone();
    let current_dir_clone2 = current_dir.clone();
    pi_hive.get_mut_property("moveup").unwrap().on_changed.connect(move |value| {
        println!("<<<< MOVE UP: {:?}", value);
        let (lock, cvar) = &*up_pair2;
        let mut going_up = lock.lock().unwrap();
        let val = value.unwrap().as_bool().unwrap();
        *going_up = val;
        current_dir_clone.store(Dir::COUNTER_CLOCKWISE, Ordering::SeqCst);
        cvar.notify_one();
    });

    pi_hive.get_mut_property("movedown").unwrap().on_changed.connect(move |value| {
        println!("<<<< MOVE DOWN: {:?}", value);
        let (lock, cvar) = &*up_pair3;
        let mut going_down = lock.lock().unwrap();
        let val = value.unwrap().as_bool().unwrap();
        *going_down = val;
        current_dir_clone2.store(Dir::CLOCKWISE, Ordering::SeqCst);
        cvar.notify_one();
    });

    let (mut sender, mut receiver) = mpsc::unbounded();

    task::spawn(async move {
        pi_hive.run().await;
    });

    motor.init();
    let mut motor_clone = motor.clone();
    // SPAWN MOTOR UP HANDLER
    task::spawn(async move {
        let (lock, cvar) = &*up_pair;
        let mut upping = lock.lock().unwrap();

        while !*upping {
            upping = cvar.wait(upping).unwrap();
            let dir = current_dir.load( Ordering::SeqCst);
            let running = motor_clone.turn(dir);
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

    // We wait here... forever
    let done = block_on(receiver.next());


    motor.done();

    println!("Done");
}
