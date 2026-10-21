//! Guard the timer dependency used by the live TUI's select loop.
use std::time::Duration;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interval_can_be_repolled_while_the_driver_fires() {
    let mut timer = tokio::time::interval(Duration::from_millis(1));
    // Another ready branch can repoll an elapsed timer before the driver
    // finishes waking it. Tokio 1.54.0 removes that timer without synchronizing
    // with the driver (M6-018), causing an assertion failure or memory corruption.
    // Keep real threads/time: a paused clock does not exercise that race.
    for _ in 0..5000 {
        loop {
            tokio::select! {
                biased;
                _ = timer.tick() => break,
                _ = tokio::task::yield_now() => {}
            }
        }
    }
}
