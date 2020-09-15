use std::{env, thread};
use std::sync::{Condvar, Mutex};
use std::sync::atomic::{AtomicU8, Ordering, AtomicBool};

use std::time::Duration;
use async_std::sync::Arc;

use futures::executor::block_on;
use hive::hive::Hive;
use local_ipaddress;

use log::{debug, info, SetLoggerError, warn};
use simple_signal::{self, Signal};

use crate::motor::Motor;

mod motor;
#[cfg(not(target_arch = "arm"))]
mod mock_gpio;

#[cfg(target_arch = "arm")]
use rppal::gpio::Level::High;
#[cfg(target_arch = "arm")]
use rppal::gpio::{Gpio};
#[cfg(not(target_arch = "arm"))]
use crate::mock_gpio::{Gpio};
#[cfg(not(target_arch = "arm"))]
use crate::mock_gpio::Level::High;


#[derive(Clone, Copy)]
pub struct GpioConfig {
    step: u8,
    dir: u8,
    power_relay_pin: u8,
    pt1:u8,
    pt2:u8,
    is_up_pin:Option<u8>,
    is_down_pin:Option<u8>,
    go_up_pin:Option<u8>,
    go_down_pin:Option<u8>,
}

pub const GPIO_HAT: GpioConfig = GpioConfig {
    step: 11,
    dir: 9,
    power_relay_pin: 10,
    pt1: 6,
    pt2: 5,
    is_up_pin: Some(2),
    is_down_pin: Some(3),
    go_up_pin: Some(18),
    go_down_pin: Some(17)
};

const GPIO_MAIN: GpioConfig = GpioConfig {
    step: 26,
    dir: 19,
    power_relay_pin: 13,
    pt1: 16,
    pt2: 20,
    is_up_pin: None,// Some(5),
    is_down_pin: None, //Some(6),
    go_up_pin: None, //Some(9),
    go_down_pin: None, //Some(11)
};

// init logging

fn init_logging() -> Result<(), SetLoggerError> {

    log4rs::init_file("log4rs.yaml", Default::default()).unwrap();
    Ok(())
}
// done init logging


struct PinDir;
impl PinDir {
    const CLOCKWISE: u8 =1;
    const COUNTER_CLOCKWISE:u8 = 0;
}


// #[non_exhaustive]
struct MoveState;
impl MoveState{
    const FREE: u8 = 0;
    const UP:u8  = 1;
    const DOWN:u8 = 2;
}


static CURRENT_DIRECTION: AtomicU8 = AtomicU8::new(0);
static CURRENT_MOVE_STATE: AtomicU8 = AtomicU8::new(MoveState::FREE);

pub fn store_direction(d:u8){
    CURRENT_DIRECTION.store(d, Ordering::Relaxed)
}
pub fn current_direction()-> u8 {
    CURRENT_DIRECTION.load(Ordering::Relaxed)
}

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

    let gpio_conf: GpioConfig = if args.contains(&str_hat) {
        info!("Running HAT config");
        GPIO_HAT
    } else {
        info!("Running MAIN config");
        GPIO_MAIN
    };

    // let step_pin:MyPin = MyPin::new(gpio_conf.step, is_test);
    // let dir_pin:MyPin = MyPin::new(gpio_conf.dir, is_test);
    // let power_pin:MyPin = MyPin::new(gpio_conf.power_relay_pin, is_test);
    // let pt_pin_1 = MyPin::new(gpio_conf.pt1, is_test);
    // let pt_pin_2 = MyPin::new(gpio_conf.pt2, is_test);

    let motor: Motor = Motor::new(gpio_conf, is_test);

    let turn_motor = move |direction:Option<u8>, do_turn:&(Mutex<bool>, Condvar)| {
        let (lock, cvar) = do_turn;
        // TODO PoisonError
        let mut turning = lock.lock().unwrap();
        let current_state = CURRENT_MOVE_STATE.load(Ordering::SeqCst);
        match direction {
            Some(PinDir::COUNTER_CLOCKWISE) => {
                if current_state == MoveState::UP {
                    info!("Already UP!!");
                } else {
                    store_direction(PinDir::COUNTER_CLOCKWISE);
                    *turning = true;
                }
            },
            Some(PinDir::CLOCKWISE) => {
                if current_state == MoveState::DOWN {
                    info!("Already DOWN!!");
                }else{
                    store_direction(PinDir::CLOCKWISE);
                    *turning = true;
                }
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
                    CURRENT_MOVE_STATE.store(MoveState::UP, Ordering::SeqCst);
                    *going_up = false;
                } else {
                    CURRENT_MOVE_STATE.store(MoveState::FREE, Ordering::SeqCst);
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
                    CURRENT_MOVE_STATE.store(MoveState::DOWN, Ordering::SeqCst);
                    *going_down = false;
                } else {
                    CURRENT_MOVE_STATE.store(MoveState::FREE, Ordering::SeqCst);
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
                    &turn_motor(Some(PinDir::COUNTER_CLOCKWISE), &*up_pair2);
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
                    &turn_motor(Some(PinDir::CLOCKWISE), &*up_pair2);
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
        let dir = if do_go_up {Some(PinDir::COUNTER_CLOCKWISE)} else {None};
        &turn_motor(dir, &*up_pair2);
    });

    let up_pair2: Arc<(Mutex<bool>, Condvar)> = up_pair.clone();
    pi_hive.get_mut_property("movedown").unwrap().on_changed.connect(move |value| {
        let do_go_down = value.unwrap().as_bool().unwrap();
        let dir = if do_go_down {Some(PinDir::CLOCKWISE)} else {None};
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
        info!("run Hive");
        block_on(pi_hive.run());
        info!("Hive run");
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


    let running = Arc::new(AtomicBool::new(true));
    simple_signal::set_handler(&[Signal::Int, Signal::Term], {
        let running = running.clone();

        move |_| {
            println!("Stopping...");
            running.store(false, Ordering::SeqCst);
        }
    });

    // Loops forever !!!
    let motor_clone = motor.clone();
    thread::spawn(move ||{
        let (lock, cvar) = &*up_pair;
        let mut turning = lock.lock().unwrap();

        while !*turning  {
            //we wait until we receive a turn message
            println!("waiting to turn");
            turning = cvar.wait(turning).unwrap();
            if *turning {
                let dir = current_direction();// CURRENT_DIRECTION.load(Ordering::SeqCst);
                motor_clone.turn(dir);

                while *turning {
                    //we wait until we receive a stop turn message
                    turning = cvar.wait(turning).unwrap();
                    if !*turning {
                        motor_clone.stop();
                        break;
                    }
                }
            }
        }
    });

    while running.load(Ordering::SeqCst) {
        // loop while were running
        thread::sleep(Duration::from_millis(100))
    };

    // Any cleanup needs to happen here
    motor.done();
    info!("Main Done");
}

// #[allow(unused_variables)]
// #[cfg(not(target_arch = "arm"))]
// fn start_input_listener(num: u8, func: impl Fn(u8) + Send + Sync + 'static) {
//     println!("starting on x86");
// }




// #[cfg(target_arch = "arm")]
fn start_input_listener(num: u8, func: impl Fn(u8) + Send + Sync + 'static) {
    thread::spawn(move || {
        let gpio = Gpio::new().unwrap();
        let mut pin = gpio.get(num).unwrap().into_input_pulldown();
        pin.set_reset_on_drop(false);
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
