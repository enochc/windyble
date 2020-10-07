use std::{env, fs, thread};
use std::sync::{Condvar, Mutex};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::time::Duration;

use async_std::sync::Arc;
use futures::executor::block_on;
use hive::hive::Hive;
use local_ipaddress;
#[allow(unused_imports)]
use log::{debug, error, info, Level, LevelFilter, Metadata, Record, SetLoggerError};
#[cfg(target_arch = "arm")]
use rppal::gpio::Gpio;
#[cfg(target_arch = "arm")]
use rppal::gpio::Level::High;
use simple_signal::{self, Signal};

#[cfg(not(target_arch = "arm"))]
use crate::mock_gpio::Gpio;
#[cfg(not(target_arch = "arm"))]
use crate::mock_gpio::Level::High;
use crate::motor::Motor;
use std::path::Path;

mod motor;
#[cfg(not(target_arch = "arm"))]
mod mock_gpio;


#[derive(Clone, Copy)]
pub struct GpioConfig {
    step: u8,
    dir: u8,
    power_relay_pin: u8,
    pt1: u8,
    pt2: u8,
    is_up_pin: Option<u8>,
    is_down_pin: Option<u8>,
    go_up_pin: Option<u8>,
    go_down_pin: Option<u8>,
}

pub const GPIO_CONF: GpioConfig = GpioConfig {
    step: 11,
    dir: 9,
    // enable motor pin
    power_relay_pin: 16,//10,
    pt1: 6,
    pt2: 5,
    is_up_pin: Some(2),
    is_down_pin: Some(3),
    go_up_pin: Some(18),
    go_down_pin: Some(17),
};

enum MotorTurnState {
    Stopped = 0,
    Go = 1,
    ReadyUp = 2,
    ReadyDown = 3,
}

impl MotorTurnState {
    fn value(&self) -> u8 {
        return match self {
            MotorTurnState::Go => 1,
            MotorTurnState::ReadyUp => 2,
            MotorTurnState::ReadyDown => 3,
            MotorTurnState::Stopped => 0,
        };
    }
}

impl PartialEq<MotorTurnState> for i8 {
    fn eq(&self, other: &MotorTurnState) -> bool {
        return match other {
            MotorTurnState::Go if self == &1 => true,
            MotorTurnState::ReadyUp if self == &2 => true,
            MotorTurnState::ReadyDown if self == &3 => true,
            MotorTurnState::Stopped if self == &0 => true,
            _ => false,
        };
    }
}

impl From<i8> for MotorTurnState {
    fn from(v: i8) -> Self {
        return match v {
            3 => MotorTurnState::ReadyDown,
            2 => MotorTurnState::ReadyUp,
            1 => MotorTurnState::Go,
            _ => MotorTurnState::Stopped,
        };
    }
}


// const GPIO_MAIN: GpioConfig = GpioConfig {
//     step: 26,
//     dir: 19,
//     power_relay_pin: 13,
//     pt1: 16,
//     pt2: 20,
//     is_up_pin: None,// Some(5),
//     is_down_pin: None, //Some(6),
//     go_up_pin: None, //Some(9),
//     go_down_pin: None, //Some(11)
// };

// init logging
pub struct SimpleLogger;

impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Debug
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            println!("{:?} - {}", record.level(), record.args());
        }
    }

    fn flush(&self) {}
}

pub static LOGGER: SimpleLogger = SimpleLogger;

fn init_logging(to_console: bool) -> Result<(), SetLoggerError> {
    if to_console {
        log::set_logger(&LOGGER).map(|()| log::set_max_level(LevelFilter::Debug))
            .expect("failed to init logger");
    } else {
        log4rs::init_file("log4rs.yaml", Default::default()).unwrap();
    }

    Ok(())
}


struct PinDir;

impl PinDir {
    const CLOCKWISE: u8 = 1;
    const COUNTER_CLOCKWISE: u8 = 0;
}


// #[non_exhaustive]
struct MoveState;

impl MoveState {
    const FREE: u8 = 0;
    const UP: u8 = 1;
    const DOWN: u8 = 2;
}


static CURRENT_DIRECTION: AtomicU8 = AtomicU8::new(0);
static CURRENT_MOVE_STATE: AtomicU8 = AtomicU8::new(MoveState::FREE);

pub fn store_direction(d: u8) {
    CURRENT_DIRECTION.store(d, Ordering::Relaxed)
}

pub fn current_direction() -> u8 {
    CURRENT_DIRECTION.load(Ordering::Relaxed)
}

#[allow(dead_code)]
fn main_test() {
    init_logging(true).expect("Failed to Init logger");
    start_input_listener(6, move |v| {
        println!("VAL {:?} is {:?}", 6, v);
    });

    let running = Arc::new(AtomicBool::new(true));
    simple_signal::set_handler(&[Signal::Int, Signal::Term], {
        let running = running.clone();

        move |sig| {
            println!("<< Received signal!! {:?}", sig);
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
///     windyble test listen 3000
///     windyble test connect 192.168.0.43:3000
/// ```
fn main() {
    /*
    pt is 0,1,2,3 potentiometer limiting for the motor 0.5 A, 1 A, 1.5 A, 2 A
    default is 2 (1.5 amps)
     */
    const INIT_PT: i64 = 2;
    let args: Vec<String> = env::args().collect();
    let to_console = args.contains(&String::from("console"));
    init_logging(to_console).expect("Failed to Init logger");
    let is_test = args.contains(&String::from("test"));
    let addr = local_ipaddress::get().unwrap();
    let props_file_name = args.get(args.len() - 1);
    let path = Path::new(props_file_name.unwrap());
    let hive_properties = if path.is_file() && !path.to_str().unwrap().contains("windyble") {
        debug!("found toml file {:?}", path);
        fs::read_to_string(path)
    } else {
        debug!("using default hive.toml");
        fs::read_to_string("hive.toml")
    };
    let properties: String = match hive_properties {
        Ok(p) => {
            p.replace("(address)", &addr)
        }
        _ => {
            error!("Failed to read hive properties file {:?}", props_file_name);
            format!("listen = \"{}:3000\"
            [Properties]
            turn: 0
            speed = {}
            pt = {}", addr, motor::DEFAULT_DURATION, INIT_PT)
        }
    };

    debug!("{}", properties);
    let mut pi_hive = Hive::new_from_str("LEFT", properties.as_str());
    let is_client: bool = !pi_hive.is_sever();

    let motor: Motor = Motor::new(GPIO_CONF, is_test);

    let turn_motor = move |direction: Option<u8>, do_turn: &(Mutex<bool>, Condvar)| {
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
            }
            Some(PinDir::CLOCKWISE) => {
                if current_state == MoveState::DOWN {
                    info!("Already DOWN!!");
                } else {
                    store_direction(PinDir::CLOCKWISE);
                    *turning = true;
                }
            }
            _ => {
                *turning = false;
            }
        }
        cvar.notify_one();
    };

    let up_pair: Arc<(Mutex<bool>, Condvar)> = Arc::new((Mutex::new(false), Condvar::new()));
    let speed_pair: Arc<(Mutex<i64>, Condvar)> = Arc::new((Mutex::new(0), Condvar::new()));
    let pt_val_pair: Arc<(Mutex<i64>, Condvar)> = Arc::new((Mutex::new(0), Condvar::new()));
    let go_direction: Arc<Mutex<MotorTurnState>> = Arc::new(Mutex::new(MotorTurnState::Stopped));

    let gpio_conf: GpioConfig = GPIO_CONF;
    if gpio_conf.is_up_pin.is_some() {
        start_input_listener(gpio_conf.is_up_pin.unwrap(), {
            let up_pair_clone = up_pair.clone();
            move |v| {
                debug!("VAL {:?} is {:?}", gpio_conf.is_up_pin, v);
                let (lock, cvar) = &*up_pair_clone;
                let mut going_up = lock.lock().unwrap();
                if v == 0 {
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

    if gpio_conf.is_down_pin.is_some() {
        start_input_listener(gpio_conf.is_down_pin.unwrap(), {
            let up_pair_clone = up_pair.clone();
            let pin_num = gpio_conf.is_down_pin.unwrap().clone();
            move |v| {
                debug!("VAL {:?} is {:?}", pin_num, v);
                let (lock, cvar) = &*up_pair_clone;
                let mut going_down = lock.lock().unwrap();
                if v == 0 {
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

    if gpio_conf.go_up_pin.is_some() {
        start_input_listener(gpio_conf.go_up_pin.unwrap(), {
            let up_pair2: Arc<(Mutex<bool>, Condvar)> = up_pair.clone();
            move |v| {
                debug!("GO UP PIN: {:?}", v);
                if v == 1 {
                    &turn_motor(Some(PinDir::COUNTER_CLOCKWISE), &*up_pair2);
                } else {
                    &turn_motor(None, &*up_pair2);
                }
            }
        });
    }

    if gpio_conf.go_down_pin.is_some() {
        start_input_listener(gpio_conf.go_down_pin.unwrap(), {
            let up_pair2: Arc<(Mutex<bool>, Condvar)> = up_pair.clone();
            move |v| {
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

    /*
        moveup and movedown are a ready go flag, 2 means, stopped, 1 means power up and get ready
        0 means to go. This is because we're bridging the stop/direction pins on the motor drivers
        so only one controller needs to run the motors and they stay perfectaly in sync. But both
        controllers need to power on the motor and prepare it to turn.
     */
    let pi_have_handle = pi_hive.get_handler();


    pi_hive.get_mut_property("turn").unwrap().on_changed.connect({
        let up_pair2: Arc<(Mutex<bool>, Condvar)> = up_pair.clone();
        let motor_clone = motor.clone();

        move |value| {
            let do_go_up = value.unwrap().as_integer().unwrap() as i8;
            if do_go_up == MotorTurnState::ReadyDown || do_go_up == MotorTurnState::ReadyUp { // Ready
                debug!("power up!");
                motor_clone.power_motor(true);
                *go_direction.lock().unwrap() = do_go_up.into();

                if is_client {
                    block_on(
                        pi_have_handle.clone()
                            .send_property_value("turn",
                                                 Some(&MotorTurnState::Go.value().into()),
                            )
                    );
                }
            } else if do_go_up == MotorTurnState::Go && !is_client { // GO
                if !is_client {
                    let direction = match *go_direction.lock().unwrap() {
                        MotorTurnState::ReadyUp => PinDir::COUNTER_CLOCKWISE,
                        _ => PinDir::CLOCKWISE
                    };
                    &turn_motor(Some(direction), &*up_pair2);
                }
            } else if do_go_up == MotorTurnState::Stopped {
                // STOP
                // the motor turns itself off
                &turn_motor(None, &*up_pair2);
                // the client doesn't power itself off because its out of the run/norun loop
                motor_clone.power_motor(false);
            }
        }
    });

    // let up_pair2: Arc<(Mutex<bool>, Condvar)> = up_pair.clone();
    // pi_hive.get_mut_property("movedown").unwrap().on_changed.connect(move |value| {
    //     let do_go_down = value.unwrap().as_bool().unwrap();
    //     let dir = if do_go_down { Some(PinDir::CLOCKWISE) } else { None };
    //     turn_motor(dir, &*up_pair2);
    // });

    let speed_clone = speed_pair.clone();
    pi_hive.get_mut_property("speed").unwrap().on_changed.connect(move |value| {
        let (lock, cvar) = &*speed_clone;
        let mut speed = lock.lock().unwrap();
        *speed = value.unwrap().as_integer().unwrap();
        cvar.notify_one();
    });

    /*
     The derived_pt is the value that was passed in via the toml text file
     which we use for initializing the motor
     */
    let derived_pt: i64 = pi_hive.properties.get("pt").unwrap()
        .value.as_ref()
        .unwrap()
        .as_integer()
        .unwrap();
    thread::spawn(move || {
        info!("run Hive");
        block_on(pi_hive.run());
    });

    motor.init(&derived_pt);

    // Handler for potentiometer
    // todo task::spawn here doesn't work.. figure out why
    let motor_clone3 = motor.clone();
    thread::spawn(move || {
        let (lock, cvar) = &*pt_val_pair;
        let mut pt = lock.lock().unwrap();
        loop {
            pt = cvar.wait(pt).unwrap();
            motor_clone3.set_potentiometer(&*pt);
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
    thread::spawn(move || {
        let (lock, cvar) = &*up_pair;
        let mut turning = lock.lock().unwrap();

        while !*turning {
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
        let mut last_val = if pin.read() == High { 1 } else { 0 };

        info!("Start listening to pin {}", num);
        loop {
            let new_val = if pin.read() == High { 1 } else { 0 };
            if new_val != last_val {
                println!("pin == {:?}", pin.read());
                func(new_val);
                last_val = new_val;
            }

            thread::sleep(Duration::from_millis(30))
        };
    });
}
