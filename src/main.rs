use std::{env, thread};
use std::sync::{Condvar, Mutex};
use std::sync::atomic::{AtomicU8, Ordering};
use std::thread::sleep;
use std::time::Duration;

use async_std::sync::Arc;
use async_std::task;
use futures::executor::block_on;
use hive::hive::Hive;
use local_ipaddress;
use sysfs_gpio::{Direction, Pin};

use crate::motor::Motor;
use crate::my_pin::MyPin;

mod motor;
mod my_pin;

const STEP: u64 = 26;
// purple
const DIR: u64 = 19;
// While
const POWER_RELAY_PIN: u64 = 13;

// Potentiometer pins
const PT1: u64 = 16;
const PT2: u64 = 20;

const IS_UP_PIN: u64 = 5;
const IS_DOWN_PIN: u64 = 6;

struct Dir;

impl Dir {
    const CLOCKWISE: u8 = 1;
    const COUNTER_CLOCKWISE: u8 = 0;
}

struct MoveState {
    free: u8,
    up: u8,
    down: u8,
}

const MOVE_STATE: MoveState = MoveState {
    free: 0,
    up: 1,
    down: 2,
};

/// Default action is to listen on 127.0.0.1:3000 unless specified otherweise
/// when connecting, it inherits properties from the server
///
/// # Examples
///
/// ```
///     board test listen 3000
///     board test connect 192.168.0.43:3000
/// ```
fn main() {
    let args: Vec<String> = env::args().collect();
    let is_test = args.contains(&String::from("test"));
    let mut action = "";
    let mut addr = local_ipaddress::get().unwrap();
    let current_move_state = Arc::new(AtomicU8::new(MOVE_STATE.free));

    for (i, name) in args.iter().enumerate() {
        if name == "connect" || name == "listen" {
            action = name;
            let adr_val = args.get(i + 1);
            if adr_val.is_none() && addr.is_empty() {
                eprintln!("No address specified for action {}", action);
                return;
            } else {
                if adr_val.is_some() {
                    if adr_val.unwrap().len() <= 6 {
                        // it's just a port number
                        addr = format!("{}:{}", addr, adr_val.unwrap());
                    } else {
                        addr = String::from(adr_val.unwrap());
                    }
                }
                println!("{}ing to: {:?}, is test: {:?}", action, addr, is_test);
                break;
            }
        }
    }

    /*
    pt is 0,1,2,3 potentiometer limiting for the motor 0.5 A, 1 A, 1.5 A, 2 A
     */
    let hive_props = format!("
    {} = {:?}
    [Properties]
    moveup = false
    movedown = false
    speed = {}
    pt = 0
    ", action, addr, motor::DEFAULT_DURATION);

    println!("{}", hive_props);

    let mut pi_hive = Hive::new_from_str("SERVE", hive_props.as_str());

    let step_pin = MyPin::new(STEP, is_test);
    let dir_pin = MyPin::new(DIR, is_test);
    let power_pin = MyPin::new(POWER_RELAY_PIN, is_test);
    let pt_pin_1 = MyPin::new(PT1, is_test);
    let pt_pin_2 = MyPin::new(PT2, is_test);
    let is_up_pin = Some(MyPin::new(IS_UP_PIN, is_test));
    let is_down_pin = Some(MyPin::new(IS_DOWN_PIN, is_test));

    let mut motor = Motor::new(
        step_pin,
        dir_pin,
        power_pin,
        pt_pin_1,
        pt_pin_2,
        is_up_pin,
        is_down_pin,
        is_test,
    );


    let up_pair: Arc<(Mutex<bool>, Condvar)> = Arc::new((Mutex::new(false), Condvar::new()));
    let speed_pair: Arc<(Mutex<i64>, Condvar)> = Arc::new((Mutex::new(0), Condvar::new()));
    let pt_val_pair: Arc<(Mutex<i64>, Condvar)> = Arc::new((Mutex::new(0), Condvar::new()));

    let current_dir: Arc<AtomicU8> = Arc::new(AtomicU8::new(0));

    let up_pair_clone = up_pair.clone();
    let current_move_state_clone = current_move_state.clone();
    start_input_listener(IS_UP_PIN, move |v| {
        println!("VAL {:?} is {:?}", IS_UP_PIN, v);
        let (lock, cvar) = &*up_pair_clone;
        let mut going_up = lock.lock().unwrap();
        if v == 1 {
            // Reached the top stop
            current_move_state_clone.store(MOVE_STATE.up, Ordering::SeqCst);
            *going_up = false;
        } else {
            current_move_state_clone.store(MOVE_STATE.free, Ordering::SeqCst);
        }
        cvar.notify_one();
    });

    let current_move_state_clone = current_move_state.clone();
    let up_pair_clone = up_pair.clone();
    start_input_listener(IS_DOWN_PIN, move |v| {
        println!("VAL {:?} is {:?}", IS_DOWN_PIN, v);
        let (lock, cvar) = &*up_pair_clone;
        let mut going_down = lock.lock().unwrap();
        if v == 1 {
            // Reached the bottom stop
            current_move_state_clone.store(MOVE_STATE.down, Ordering::SeqCst);
            *going_down = false;
        } else {
            current_move_state_clone.store(MOVE_STATE.free, Ordering::SeqCst);
        }
        cvar.notify_one();
    });

    let pt_val_clone: Arc<(Mutex<i64>, Condvar)> = pt_val_pair.clone();
    pi_hive.get_mut_property("pt").unwrap().on_changed.connect(move |value| {
        let (lock, cvar) = &*pt_val_clone;
        let mut pt = lock.lock().unwrap();
        *pt = value.unwrap().as_integer().unwrap();
        cvar.notify_one();
    });

    let up_pair2: Arc<(Mutex<bool>, Condvar)> = up_pair.clone();
    let current_dir_clone: Arc<AtomicU8> = current_dir.clone();
    let current_move_state_clone = current_move_state.clone();
    pi_hive.get_mut_property("moveup").unwrap().on_changed.connect(move |value| {
        if current_move_state_clone.load(Ordering::SeqCst) == MOVE_STATE.up {
            println!("Already UP!!");
            return;
        }
        let (lock, cvar) = &*up_pair2;
        let mut going_up = lock.lock().unwrap();
        *going_up = value.unwrap().as_bool().unwrap();
        current_dir_clone.store(Dir::COUNTER_CLOCKWISE, Ordering::SeqCst);
        cvar.notify_one();
    });

    let up_pair3: Arc<(Mutex<bool>, Condvar)> = up_pair.clone();
    let current_dir_clone2: Arc<AtomicU8> = current_dir.clone();
    let current_move_state_clone = current_move_state.clone();
    pi_hive.get_mut_property("movedown").unwrap().on_changed.connect(move |value| {
        println!("Current Move statet: {:?}", current_move_state_clone);
        if current_move_state_clone.load(Ordering::SeqCst) == MOVE_STATE.down {
            println!("Already DOWN!!");
            return;
        }
        let (lock, cvar) = &*up_pair3;
        let mut going_down = lock.lock().unwrap();
        *going_down = value.unwrap().as_bool().unwrap();
        current_dir_clone2.store(Dir::CLOCKWISE, Ordering::SeqCst);
        cvar.notify_one();
    });

    let speed_clone = speed_pair.clone();
    pi_hive.get_mut_property("speed").unwrap().on_changed.connect(move |value| {
        let (lock, cvar) = &*speed_clone;
        let mut speed = lock.lock().unwrap();
        *speed = value.unwrap().as_integer().unwrap();
        cvar.notify_one();
    });

    thread::spawn(move || {
        block_on(pi_hive.run());
    });

    motor.init();

    // Handler for potentiometer
    let motor_clone3 = motor.clone();
    task::spawn(async move {
        let (lock, cvar) = &*pt_val_pair;
        let mut pt = lock.lock().unwrap();
        loop {
            pt = cvar.wait(pt).unwrap();
            motor_clone3.set_potentiometer(*pt);
        }
    });

    // Handler for speed
    // todo task::spawn here doesn't work.. figure out why
    let motor_clone2 = motor.clone();
    thread::spawn(move || {
        let (lock, cvar) = &*speed_pair;
        let mut speed = lock.lock().unwrap();
        loop {
            speed = cvar.wait(speed).unwrap();
            motor_clone2.set_speed(*speed as u64);
        };
    });

    let (lock, cvar) = &*up_pair;
    let mut turning = lock.lock().unwrap();

    // Loops forever !!!
    // let mut motor_clone = motor.clone();
    while !*turning {
        //we wait until we receive a turn message
        turning = cvar.wait(turning).unwrap();
        if *turning {
            let dir = current_dir.load(Ordering::SeqCst);
            let running = motor.turn(dir);

            println!("<< TURNING {:?}", turning);
            while *turning {
                //we wait until we receive a stop turn message
                turning = cvar.wait(turning).unwrap();
                if !*turning {
                    motor.stop();
                    running.unwrap().store(false, Ordering::SeqCst);
                }

                break;
            }
        }
    }

    // this never runs, the pins are never exported because the only way to end this loop
    // Is to kill the service
    motor.done();

    println!("Main Done");
}

// TODO this works well enough as is, but is not the best solution. preferably we should
//  start a single thread and run each listeners in a task instead of starting a new thread
//  for every input
fn start_input_listener(num: u64, func: impl Fn(u8) + Send + Sync + 'static) {
    thread::spawn(move || {
        let input = Pin::new(num);
        input.with_exported(|| {
            input.set_direction(Direction::In)?;
            let mut prev_val: u8 = 255;
            loop {
                let val = input.get_value()?;
                if val != prev_val {
                    prev_val = val;
                    func(val);
                }
                sleep(Duration::from_millis(10));
            }
        })
    });
}
