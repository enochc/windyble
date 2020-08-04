extern crate sysfs_gpio;
use futures::channel::{mpsc, mpsc::UnboundedSender, mpsc::UnboundedReceiver};
use futures::executor::block_on;
use sysfs_gpio::{Direction, Pin};
use std::thread::sleep;
use std::time::Duration;
use hive::hive::Hive;
use futures::{SinkExt, StreamExt};
use async_std::task;

fn main() {
    let hive_props = r#"
    listen = "192.168.5.48:3000"
    [Properties]
    light = 1
    "#;

    let mut pi_hive = Hive::new_from_str("SERVE", hive_props);
    let my_led = Pin::new(26);
    my_led.with_exported(|| {


        pi_hive.get_mut_property("light").unwrap().on_changed.connect(move |value|{
            println!("<<<< LIGHT CHANGED: {:?}", value);
            let val = value.unwrap().as_integer().unwrap();
            let lightVal = match val {
                v if v >0 => 1,
                _ => 0,
            };
            my_led.set_value(lightVal).unwrap();
            // my_led.with_exported(|| {
            //     my_led.set_direction(Direction::Out).unwrap();
            //     for x in 0..9 {
            //         my_led.set_value(0).unwrap();
            //         sleep(Duration::from_millis(200));
            //         my_led.set_value(1).unwrap();
            //         sleep(Duration::from_millis(200));
            //     }
            //     Ok(())
            // }).unwrap();
        });
        let (mut send_chan, mut receive_chan) = mpsc::unbounded();
        task::spawn(async move {
            pi_hive.run().await;
            send_chan.send(true).await;
        });

        let done = block_on(receive_chan.next());
        println!("Done");

        Ok(())
    }).unwrap();


}
