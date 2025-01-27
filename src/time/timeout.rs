// Code based heavily on [embassy-time](https://github.com/embassy-rs/embassy/blob/main/embassy-time/src/timer.rs),
// which is released jointly under the Apache 2.0 and MIT licenses.

use core::{future::Future, pin::pin};

use super::Duration;
use embassy_futures::select::{select, Either};
use embedded_hal_async::delay::DelayNs;
use nrf52833_hal::{timer::Instance as TimerInstance, Timer as HALTimer};

/// Error returned by [`with_timeout`] and [`with_deadline`] on timeout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeoutError;

/// Runs a given future with a timeout.
///
/// If the future completes before the timeout, its output is returned. Otherwise, on timeout,
/// work on the future is stopped (`poll` is no longer called), the future is dropped and `Err(TimeoutError)` is returned.
pub async fn with_timeout<T: TimerInstance, U, F: Future>(
    timer: &mut HALTimer<T, U>,
    timeout: Duration,
    fut: F,
) -> Result<F::Output, TimeoutError> {
    let timeout_fut = timer.delay_ms(timeout.as_micros() as u32);
    match select(pin!(fut), timeout_fut).await {
        Either::First(r) => Ok(r),
        Either::Second(_) => Err(TimeoutError),
    }
}

/// Provides functions to run a given future with a timeout or a deadline.
pub trait WithTimeout {
    /// Output type of the future.
    type Output;

    /// Runs a given future with a timeout.
    ///
    /// If the future completes before the timeout, its output is returned. Otherwise, on timeout,
    /// work on the future is stopped (`poll` is no longer called), the future is dropped and `Err(TimeoutError)` is returned.
    async fn with_timeout<T: TimerInstance, U>(
        self,
        timer: &mut HALTimer<T, U>,
        timeout: Duration,
    ) -> Result<Self::Output, TimeoutError>;
}

impl<F: Future> WithTimeout for F {
    type Output = F::Output;

    async fn with_timeout<T: TimerInstance, U>(
        self,
        timer: &mut HALTimer<T, U>,
        timeout: Duration,
    ) -> Result<Self::Output, TimeoutError> {
        with_timeout(timer, timeout, self).await
    }
}
