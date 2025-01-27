#![no_std]
#![no_main]

mod matrix;
mod time;

use core::ops::{Deref, DerefMut};
#[allow(unused_imports)]
use defmt_rtt as _;
#[allow(unused_imports)]
use panic_probe as _;

use crate::matrix::Matrix;
use defmt::info;
use embassy_executor::Spawner;
use embassy_futures::join::join3;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embedded_hal::digital::OutputPin;
use embedded_hal_async::{delay::DelayNs, digital::Wait};
use nrf52833_hal::gpio::{Input, Output, Pin};
use nrf52833_hal::{self as hal, gpio::Level, Timer};

#[derive(Debug, Clone, Copy)]
enum TrafficFlowState {
    NorthSouth,
    EastWest,
    North,
    South,
    East,
    West,
    None,
}

#[derive(Debug, Clone, Copy)]
enum TrafficLightState {
    Stop,
    Ready,
    Go,
    Slow,
}

impl TrafficLightState {
    fn next(&self) -> TrafficLightState {
        match self {
            TrafficLightState::Stop => TrafficLightState::Ready,
            TrafficLightState::Ready => TrafficLightState::Go,
            TrafficLightState::Go => TrafficLightState::Slow,
            TrafficLightState::Slow => TrafficLightState::Stop,
        }
    }
}

struct TrafficLight {
    state: Mutex<CriticalSectionRawMutex, TrafficLightState>,
    col: u8,
}

impl TrafficLight {
    fn new(col: u8) -> Self {
        TrafficLight {
            state: Mutex::new(TrafficLightState::Stop),
            col,
        }
    }

    async fn run<T: hal::timer::Instance>(&self, timer: &mut Timer<T>) {
        loop {
            match self.state().await {
                TrafficLightState::Stop => {
                    info!("Stop");
                    timer.delay_ms(500).await;
                }
                TrafficLightState::Ready => {
                    info!("Ready");
                    timer.delay_ms(500).await;
                }
                TrafficLightState::Go => {
                    info!("Go");
                    timer.delay_ms(500).await;
                }
                TrafficLightState::Slow => {
                    info!("Slow");
                    timer.delay_ms(500).await;
                }
            }

            {
                let mut guard = self.state.lock().await;
                let s = guard.deref_mut();
                *s = s.next();
            }
        }
    }

    async fn show(&self, matrix: &Matrix) {
        info!("Clearing col");
        matrix.clear_col(self.col).await;
        match self.state().await {
            TrafficLightState::Stop => {
                info!("RED");
                matrix.set(0, self.col, true).await;
                info!("after STOP");
            }
            TrafficLightState::Ready => {
                info!("RED");
                matrix.set(0, self.col, true).await;
                info!("AMBER");
                matrix.set(1, self.col, true).await;
                info!("after READY");
            }
            TrafficLightState::Go => {
                info!("GREEN");
                matrix.set(2, self.col, true).await;
                info!("after GO");
            }
            TrafficLightState::Slow => {
                info!("AMBER");
                matrix.set(1, self.col, true).await;
                info!("after SLOW");
            }
        }
        info!("Done showing traffic light");
    }

    async fn state(&self) -> TrafficLightState {
        *self.state.lock().await.deref()
    }
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("Starting...");

    let p = hal::pac::Peripherals::take().unwrap();

    let port0 = hal::gpio::p0::Parts::new(p.P0);
    let port1 = hal::gpio::p1::Parts::new(p.P1);

    let mut timer0 = Timer::new(p.TIMER0);
    let timer1 = Timer::new(p.TIMER1);
    let mut timer2 = Timer::new(p.TIMER2);

    let traffic_light_north = TrafficLight::new(0);

    let matrix = Matrix::new(
        port0.p0_21.into_push_pull_output(Level::High),
        port0.p0_22.into_push_pull_output(Level::High),
        port0.p0_15.into_push_pull_output(Level::High),
        port0.p0_24.into_push_pull_output(Level::High),
        port0.p0_19.into_push_pull_output(Level::High),
        port0.p0_28.into_push_pull_output(Level::Low),
        port0.p0_11.into_push_pull_output(Level::Low),
        port0.p0_31.into_push_pull_output(Level::Low),
        port1.p1_05.into_push_pull_output(Level::Low),
        port0.p0_30.into_push_pull_output(Level::Low),
    );

    join3(
        traffic_light_north.run(&mut timer0),
        matrix.run(timer1),
        async {
            loop {
                info!("Showing traffic light");
                traffic_light_north.show(&matrix).await;
                timer2.delay_ns(10).await;
            }
        },
    )
    .await;
}
