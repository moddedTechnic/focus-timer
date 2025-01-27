#![no_std]
#![no_main]

mod matrix;

#[allow(unused_imports)]
use defmt_rtt as _;
#[allow(unused_imports)]
use panic_probe as _;

use crate::matrix::Matrix;
use defmt::info;
use embassy_executor::Spawner;
use embedded_hal_async::delay::DelayNs;
use nrf52833_hal::{self as hal, Rtc};
use nrf_time::{initialize_timer, Instant, Timer};

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("Starting...");

    let mut cp = hal::pac::CorePeripherals::take().unwrap();
    let p = hal::pac::Peripherals::take().unwrap();

    let clocks = hal::clocks::Clocks::new(p.CLOCK);
    let _clocks = clocks.start_lfclk();

    let rtc = Rtc::new(p.RTC2, 0).unwrap();
    initialize_timer!(RTC2 from (rtc, &mut cp.NVIC));

    loop {
        info!("Time: {}", Instant::now());
        Timer::after_millis(100).await;
    }
}
