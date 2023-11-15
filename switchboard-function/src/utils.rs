use crate::*;

use std::time::Duration;
use tokio::time::{ interval, Interval };
use std::result::Result;

pub fn get_market_name_bytes(s: &str) -> [u8; 8] {
    let mut b = [0u8; 8];
    let bytes = s.as_bytes();
    b[0..bytes.len()].copy_from_slice(bytes);
    b
}

pub async fn start_routine<F, Fut>(routine_interval: u64, async_fn: F) -> Result<(), SbError>
    where F: FnMut() -> Fut, Fut: std::future::Future<Output = Result<(), SbError>>
{
    let mut interval: Interval = interval(Duration::from_secs(std::cmp::max(1, routine_interval)));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    start_routine_from_interval(interval, async_fn).await
}

pub async fn start_routine_from_interval<F, Fut>(
    mut interval: Interval,
    mut async_fn: F
)
    -> Result<(), SbError>
    where F: FnMut() -> Fut, Fut: std::future::Future<Output = Result<(), SbError>>
{
    // let counter = Arc::new(Mutex::new(1));

    loop {
        interval.tick().await; // This waits for the next tick (every 1 second)
        // let current_counter = {
        //     let mut locked_counter = counter.lock().await;
        //     let val = *locked_counter;
        //     *locked_counter += 1;
        //     val
        // };

        // Run custom async fn here
        async_fn().await?;
    }
}
