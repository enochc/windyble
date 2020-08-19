    //
    //
    // use async_mutex::Mutex;
    // use smol::Task;
    // use std::sync::Arc;
    // // use futures::executor::block_on;
    //
    // async fn doit(){
    //     let m = Arc::new(Mutex::new(0));
    //     let mut tasks = vec![];
    //
    //     for _ in 0..10 {
    //         let m = m.clone();
    //         tasks.push(smol::Task::spawn(async move {
    //             *m.lock().await += 1;
    //
    //         }));
    //     }
    //
    //     for t in tasks {
    //         t.await;
    //     }
    //     assert_eq!(*m.lock().await, 10);
    // }


fn main() {
    // block_on(doit());
}