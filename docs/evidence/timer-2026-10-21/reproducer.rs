use std::time::Duration;
#[tokio::main(worker_threads = 2)]
async fn main() {
    let mut timer = tokio::time::interval(Duration::from_millis(1));
    for _ in 0..5000 {
        loop {
            tokio::select! {
                biased;
                _ = timer.tick() => break,
                _ = tokio::task::yield_now() => {}
            }
        }
    }
    println!("5000 ticks passed");
}
