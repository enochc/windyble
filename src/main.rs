extern crate sysfs_gpio;
use sysfs_gpio::{Direction, Pin};
use std::thread::{sleep, sleep_ms};
use std::time::Duration;
use hive::hive::Hive;
use async_std::task;
use async_std::sync::Arc;
use std::sync::atomic::{Ordering, AtomicU8};


fn main() {
    let step = 26;
    let dir = 19;

    // let hive_props = r#"
    // listen = "192.168.5.41:3000"
    // [Properties]
    // light = false
    // "#;

    // let mut pi_hive = Hive::new_from_str("SERVE", hive_props);
    // let my_led = Pin::new(26);
    let step_pin = Pin::new(step);
    let dir_pin = Pin::new(dir);

    let turn_delay = Duration::from_micros(1000);

    // dir_pin.with_exported(||{
    //     dir_pin.set_direction(Direction::Out)
    // })
    //     .expect(format!("Failed to set direction on dir pin: ({:?})",dir).as_str());

    // step_pin.unexport().expect("Failed to Unexport step");
    dir_pin.export().expect("Failed to export dir pin");
    step_pin.with_exported(move|| {
        // Sleep a moment to allow the pin privileges to update
        sleep(Duration::from_millis(80));
        step_pin.set_direction(Direction::Out)
            .expect(format!("Failed to set direction on step pin: ({:?})",step_pin.get_pin_num()).as_str());

        for _x in 0..200 {
            step_pin.set_value(1).unwrap();
            sleep(turn_delay);
            step_pin.set_value(0).unwrap();
            sleep(turn_delay);
        }
        Ok(())
    }).expect("Failed to turn motor");
    dir_pin.unexport().expect("Failed to unexport direction pin");



    // let light_value = Arc::new(AtomicU8::new(0));
    //
    // let hive_change_light_value = light_value.clone();
    // //TODO make onchanged.connect an FnMut so I can pass in a channel that sends a value
    // pi_hive.get_mut_property("light").unwrap().on_changed.connect(move |value|{
    //     println!("<<<< LIGHT CHANGED: {:?}", value);
    //     let val = value.unwrap().as_bool().unwrap();
    //     let tmp_val = if val {1}else{0};
    //     hive_change_light_value.store(tmp_val, Ordering::SeqCst);
    //
    // });
    //
    //
    // task::spawn(async move {
    //     pi_hive.run().await.expect("Have failed");
    // });
    //
    // let led_loop_value = light_value.clone();
    // my_led.set_direction(Direction::Out).unwrap();
    // my_led.with_exported(move|| {
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
