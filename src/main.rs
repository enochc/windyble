use std::{env, thread};
use std::sync::{Condvar, Mutex};
use std::sync::atomic::{AtomicU8, Ordering, AtomicBool};

use std::time::Duration;
use async_std::sync::Arc;
use async_std::task;


use futures::executor::block_on;
use hive::hive::Hive;
use local_ipaddress;
use log::{debug, info, LevelFilter, SetLoggerError, warn};
use log::{Level, Metadata, Record};
use simple_signal::{self, Signal};

use crate::motor::Motor;
use crate::my_pin::MyPin;

mod motor;
mod my_pin;

#[derive(Clone, Copy)]
pub struct GpioConfig {
    step: u64,
    dir: u64,
    power_relay_pin: u64,
    pt1:u64,
    pt2:u64,
    is_up_pin:Option<u8>,
    is_down_pin:Option<u8>,
    go_up_pin:Option<u8>,
    go_down_pin:Option<u8>,
}

const GPIO_HAT: GpioConfig = GpioConfig {
    step: 11,
    dir: 9,
    power_relay_pin: 10,
    pt1: 6,
    pt2: 5,
    is_up_pin: None,
    is_down_pin: None,
    go_up_pin: None,
    go_down_pin: None
};

const GPIO_MAIN: GpioConfig = GpioConfig {
    step: 26,
    dir: 19,
    power_relay_pin: 13,
    pt1: 16,
    pt2: 20,
    is_up_pin: Some(5),
    is_down_pin: Some(6),
    go_up_pin: Some(9),
    go_down_pin: Some(11)
};

// const STEP: u64 = 11;//26;
// const DIR: u64 = 9;//19;
// const POWER_RELAY_PIN: u64 = 10;//13;
//
// // Potentiometer pins
// const PT1: u64 = 6;//16;
// const PT2: u64 = 5;//20;
//
// // delimiter pins
// const IS_UP_PIN: Option<u64> = None;//Some(5);
// const IS_DOWN_PIN: Option<u64> = None;//Some(6);
//
// //physical up/down pins
// const GO_UP_PIN:Option<u64> = None;//Some(9);
// const GO_DOWN_PIN:Option<u64> = None;//Some(11);

// init logging
pub struct SimpleLogger;
impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Debug
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            // println!("{:?}{:?}, {:?} - {}", record.file(), record.line(), record.level(), record.args());
            println!("{:?} - {}", record.level(), record.args());
        }
    }

    fn flush(&self) {}
}
pub static LOGGER: SimpleLogger = SimpleLogger;

fn init_logging() -> Result<(), SetLoggerError> {
    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(LevelFilter::Debug))
}
// done init logging

struct Dir;

impl Dir {
    const CLOCKWISE: u8 = 1; // DOWN
    const COUNTER_CLOCKWISE: u8 = 0; // UO
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

static CURRENT_DIRECTION: AtomicU8 = AtomicU8::new(0);
static CURRENT_MOVE_STATE: AtomicU8 = AtomicU8::new(MOVE_STATE.free);


#[allow(dead_code)]
fn main_test() {
    init_logging().expect("Failed to Init logger");
    start_input_listener(6, move |v| {
        println!("VAL {:?} is {:?}", 6, v);

    });

    let running = Arc::new(AtomicBool::new(true));
    simple_signal::set_handler(&[Signal::Int, Signal::Term], {
        let running = running.clone();

        move |sig| {
            println!("<< Received signal!! {:?}",sig);
            running.store(false, Ordering::SeqCst);
        }
    });
    while running.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_secs(1));
    }
    println!("all done");
}

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


    init_logging().expect("Failed to Init logger");

    let args: Vec<String> = env::args().collect();
    let str_hat = String::from("hat");
    let gpio_conf: GpioConfig = if args.contains(&str_hat) {
        info!("Running HAT config");
        GPIO_HAT
    } else {
        info!("Running MAIN config");
        GPIO_MAIN
    };
    let is_test = args.contains(&String::from("test"));
    let mut action = "";
    let mut addr = local_ipaddress::get().unwrap();
    // let current_move_state: Arc<AtomicU8> = Arc::new(AtomicU8::new(MOVE_STATE.free));

    let mut found:bool = false;
    for (i, name) in args.iter().enumerate() {
        if name == "connect" || name == "listen" {
            found = true;
            action = name;
            let adr_val = args.get(i + 1);
            if adr_val.is_none() && addr.is_empty() {
                warn!("No address specified for action {}", action);
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
                info!("{}ing to: {:?}, is test: {:?}", action, addr, is_test);
                break;
            }
        }
    }
    if !found { // connect or listen not specified
        // default to listen 3000
        action = "listen";
        addr = format!("{}:3000", addr);
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

    info!("{}", hive_props);


    let mut pi_hive = Hive::new_from_str("SERVE", hive_props.as_str());

    let step_pin = MyPin::new(gpio_conf.step, is_test);
    let dir_pin = MyPin::new(gpio_conf.dir, is_test);
    let power_pin = MyPin::new(gpio_conf.power_relay_pin, is_test);
    let pt_pin_1 = MyPin::new(gpio_conf.pt1, is_test);
    let pt_pin_2 = MyPin::new(gpio_conf.pt2, is_test);

    let mut motor = Motor::new(
        step_pin,
        dir_pin,
        power_pin,
        pt_pin_1,
        pt_pin_2,
        is_test,
    );

    let turn_motor = move |direction:Option<u8>, do_turn:&(Mutex<bool>, Condvar)| {
        let (lock, cvar) = do_turn;
        let mut turning = lock.lock().unwrap();
        let current_state = CURRENT_MOVE_STATE.load(Ordering::SeqCst);
        match direction {
            Some(dir) => {
                if dir == Dir::COUNTER_CLOCKWISE && current_state == MOVE_STATE.up {
                    info!("Already UP!!");
                    return;
                } else if dir == Dir::CLOCKWISE && current_state == MOVE_STATE.down {
                    info!("Already DOWN!!");
                    return;
                }

                CURRENT_DIRECTION.store(dir, Ordering::SeqCst);
                *turning = true;
            },
            _ => {
                *turning = false;
            }
        }
        cvar.notify_one();
    };

    let up_pair: Arc<(Mutex<bool>, Condvar)> = Arc::new((Mutex::new(false), Condvar::new()));
    let speed_pair: Arc<(Mutex<i64>, Condvar)> = Arc::new((Mutex::new(0), Condvar::new()));
    let pt_val_pair: Arc<(Mutex<i64>, Condvar)> = Arc::new((Mutex::new(0), Condvar::new()));

    if gpio_conf.is_up_pin.is_some(){
        start_input_listener(gpio_conf.is_up_pin.unwrap(),  {
            let up_pair_clone = up_pair.clone();
            move |v| {
                debug!("VAL {:?} is {:?}", gpio_conf.is_up_pin, v);
                let (lock, cvar) = &*up_pair_clone;
                let mut going_up = lock.lock().unwrap();
                if v == 1 {
                    // Reached the top stop
                    CURRENT_MOVE_STATE.store(MOVE_STATE.up, Ordering::SeqCst);
                    *going_up = false;
                } else {
                    CURRENT_MOVE_STATE.store(MOVE_STATE.free, Ordering::SeqCst);
                }
                cvar.notify_one();
            }
        });
    }

    if gpio_conf.is_down_pin.is_some(){
        start_input_listener(gpio_conf.is_down_pin.unwrap(), {
            let up_pair_clone = up_pair.clone();
            let pin_num = gpio_conf.is_down_pin.unwrap().clone();
            move |v| {
                debug!("VAL {:?} is {:?}", pin_num, v);
                let (lock, cvar) = &*up_pair_clone;
                let mut going_down = lock.lock().unwrap();
                if v == 1 {
                    // Reached the bottom stop
                    CURRENT_MOVE_STATE.store(MOVE_STATE.down, Ordering::SeqCst);
                    *going_down = false;
                } else {
                    CURRENT_MOVE_STATE.store(MOVE_STATE.free, Ordering::SeqCst);
                }
                cvar.notify_one();
            }
        });
    }

    if gpio_conf.go_up_pin.is_some(){
        start_input_listener(gpio_conf.go_up_pin.unwrap(), {
            let up_pair2: Arc<(Mutex<bool>, Condvar)> = up_pair.clone();
            move|v|{
                debug!("GO UP PIN: {:?}", v);
                if v == 1 {
                    &turn_motor(Some(Dir::COUNTER_CLOCKWISE), &*up_pair2);
                } else {
                    &turn_motor(None, &*up_pair2);
                }
            }
        });
    }

    if gpio_conf.go_down_pin.is_some(){
        start_input_listener(gpio_conf.go_down_pin.unwrap(), {
            let up_pair2: Arc<(Mutex<bool>, Condvar)> = up_pair.clone();
            move|v|{
                debug!("GO DOWN PIN: {:?}", v);
                if v == 1 {
                    &turn_motor(Some(Dir::CLOCKWISE), &*up_pair2);
                } else {
                    &turn_motor(None, &*up_pair2);
                }
            }
        });
    }


    let pt_val_clone: Arc<(Mutex<i64>, Condvar)> = pt_val_pair.clone();
    pi_hive.get_mut_property("pt").unwrap().on_changed.connect(move |value| {
        let (lock, cvar) = &*pt_val_clone;
        let mut pt = lock.lock().unwrap();
        *pt = value.unwrap().as_integer().unwrap();
        cvar.notify_one();
    });

    let up_pair2: Arc<(Mutex<bool>, Condvar)> = up_pair.clone();
    pi_hive.get_mut_property("moveup").unwrap().on_changed.connect(move |value| {
        let do_go_up = value.unwrap().as_bool().unwrap();
        let dir = if do_go_up {Some(Dir::COUNTER_CLOCKWISE)} else {None};
        &turn_motor(dir, &*up_pair2);
    });

    let up_pair2: Arc<(Mutex<bool>, Condvar)> = up_pair.clone();
    pi_hive.get_mut_property("movedown").unwrap().on_changed.connect(move |value| {
        let do_go_down = value.unwrap().as_bool().unwrap();
        let dir = if do_go_down {Some(Dir::CLOCKWISE)} else {None};
        turn_motor(dir, &*up_pair2);
    });

    let speed_clone = speed_pair.clone();
    pi_hive.get_mut_property("speed").unwrap().on_changed.connect(move |value| {
        let (lock, cvar) = &*speed_clone;
        let mut speed = lock.lock().unwrap();
        *speed = value.unwrap().as_integer().unwrap();
        cvar.notify_one();
    });

    thread::spawn(move || {
        println!("run Hive");
        block_on(pi_hive.run());
        println!("Hive run");
    });

    motor.init();

    // Handler for potentiometer
    // todo task::spawn here doesn't work.. figure out why
    let motor_clone3 = motor.clone();
    thread::spawn( move ||{
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
        println!("waiting to turn");
        turning = cvar.wait(turning).unwrap();
        if *turning {
            let dir = CURRENT_DIRECTION.load(Ordering::SeqCst);
            let running = motor.turn(dir);

            while *turning {
                //we wait until we receive a stop turn message
                turning = cvar.wait(turning).unwrap();
                if !*turning {
                    motor.stop();
                    running.unwrap().store(false, Ordering::SeqCst);
                    break;
                }


            }
        }
    }

    // this never runs, the pins are never exported because the only way to end this loop
    // Is to kill the service
    motor.done();
    info!("Main Done");
}

#[allow(unused_variables)]
#[cfg(target_arch = "x86_64")]
fn start_input_listener(num: u8, func: impl Fn(u8) + Send + Sync + 'static) {
    println!("starting on non x86");
}

#[cfg(target_arch = "arm")]
use sysfs_gpio::{Direction, Pin};

#[cfg(target_arch = "arm")]
use rppal::gpio::{Gpio, Trigger};
#[cfg(target_arch = "arm")]
use rppal::gpio::Level::High;

#[cfg(target_arch = "arm")]
// TODO this works well enough as is, but is not the best solution. preferably we should
//  start a single thread and run each listeners in a task instead of starting a new thread
//  for every input
fn start_input_listener(num: u8, func: impl Fn(u8) + Send + Sync + 'static) {
    thread::spawn(move || {
        let gpio = Gpio::new().unwrap();
        let pin = gpio.get(num).unwrap().into_input_pulldown();
        let mut last_val = if pin.read() == High {1} else {0};

        info!("Start listening to pin {}", num);
        loop {
            let new_val = if pin.read() == High {1} else {0};
            if new_val != last_val {
                println!("pin == {:?}", pin.read());
                func(new_val);
                last_val = new_val;
            }

            thread::sleep(Duration::from_millis(30))
        };


    });
}

// orig
// fn start_input_listener(num: u64, func: impl Fn(u8) + Send + Sync + 'static) {
//     thread::spawn(move || {
//         info!("Start listening to pin {}", num);
//         let input = Pin::new(num);
//         input.with_exported(|| {
//             // the sleep here is a workaround on an async bug in the pin export code.
//             thread::sleep(Duration::from_millis(100));
//             input.set_active_low(true).expect("Failed to set active low");
//             input.set_direction(Direction::In)?;
//             let mut prev_val: u8 = 255;
//             loop {
//                 let val = input.get_value()?;
//                 if val != prev_val {
//                     info!("<< input changed: on{} to {}", num, val);
//                     prev_val = val;
//                     func(val);
//                 }
//                 thread::sleep(Duration::from_millis(30));
//             }
//         })
//     });
// }
