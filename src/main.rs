mod motor;
mod my_pin;

use hive::hive::Hive;
use async_std::task;
use async_std::sync::{Arc};
use std::sync::{Condvar, Mutex};
use std::sync::atomic::{Ordering, AtomicU8};

use futures::channel::mpsc;
use futures::{SinkExt, StreamExt};
use futures::executor::block_on;
use crate::my_pin::MyPin;
use crate::motor::Motor;
use std::{thread, env};


const STEP: u64 = 26; // purple
const DIR: u64 = 19; //While

struct Dir;

impl Dir {
    const CLOCKWISE: u8 = 1;
    const COUNTER_CLOCKWISE: u8 = 0;
}


fn main() {
    let args: Vec<String> = env::args().collect();
    let is_test = args.contains(&String::from("test"));

    let addr = if is_test { "127.0.0.1:3000" } else { "192.168.5.41:3000" };

    let hive_props = format!("
    listen = {:?}
    [Properties]
    moveup = false
    movedown = false
    speed = 1000
    ", addr);

    let mut pi_hive = Hive::new_from_str("SERVE", hive_props.as_str());

    let step_pin = MyPin::new(STEP, is_test);
    let dir_pin = MyPin::new(DIR, is_test);

    let motor = Motor::new(step_pin, dir_pin, is_test);

    // let move_up_clone = move_up.clone();
    let up_pair = Arc::new((Mutex::new(false), Condvar::new()));
    let up_pair2 = up_pair.clone();
    let up_pair3 = up_pair.clone();

    let speed_pair = Arc::new((Mutex::new(0), Condvar::new()));
    let speed_clone = speed_pair.clone();

    let current_dir = Arc::new(AtomicU8::new(0));
    let current_dir_clone = current_dir.clone();
    let current_dir_clone2 = current_dir.clone();
    pi_hive.get_mut_property("moveup").unwrap().on_changed.connect(move |value| {
        let (lock, cvar) = &*up_pair2;
        let mut going_up = lock.lock().unwrap();
        let val = value.unwrap().as_bool().unwrap();
        *going_up = val;
        current_dir_clone.store(Dir::COUNTER_CLOCKWISE, Ordering::SeqCst);
        cvar.notify_one();
    });

    pi_hive.get_mut_property("movedown").unwrap().on_changed.connect(move |value| {
        let (lock, cvar) = &*up_pair3;
        let mut going_down = lock.lock().unwrap();
        let val = value.unwrap().as_bool().unwrap();
        *going_down = val;
        current_dir_clone2.store(Dir::CLOCKWISE, Ordering::SeqCst);
        cvar.notify_one();
    });

    pi_hive.get_mut_property("speed").unwrap().on_changed.connect(move |value| {
        let (lock, cvar) = &*speed_clone;
        let mut speed = lock.lock().unwrap();
        let val = value.unwrap().as_integer().unwrap();
        *speed = val;
        cvar.notify_one();
    });

    task::spawn(async move {
        pi_hive.run().await;
    });

    motor.init();
    let mut motor_clone = motor.clone();
    let mut motor_clone2 = motor_clone.clone();

    // Handler for speed
    let _ = thread::spawn(move || {
        let (lock, cvar) = &*speed_pair;
        let mut speed = lock.lock().unwrap();
        loop {
            speed = cvar.wait(speed).unwrap();
            motor_clone2.set_speed(*speed as u64);
        };
    });

    let (mut sender, mut receiver) = mpsc::unbounded();

    task::spawn(async move {
        let (lock, cvar) = &*up_pair;
        let mut upping = lock.lock().unwrap();

        while !*upping {
            upping = cvar.wait(upping).unwrap();
            let dir = current_dir.load(Ordering::SeqCst);
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
        // TODO why does appenging an await on the line below, break everything?
        sender.send(1);
    });

    // We wait here... forever
    let done = block_on(receiver.next());
    assert_eq!(1, done.unwrap());


    motor.done();

    println!("Done");
}
