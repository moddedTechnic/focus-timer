#![no_std]
#![no_main]

#[allow(unused_imports)]
use defmt_rtt as _;
#[allow(unused_imports)]
use nrf52833_hal as _;
#[allow(unused_imports)]
use panic_probe as _;

use core::{
    cell::RefCell,
    sync::atomic::{AtomicBool, Ordering},
};
use cortex_m::interrupt::Mutex;
use defmt::info;
use embassy_executor::Spawner;
use embassy_futures::select::{select, Either};
use embedded_hal_async::digital::Wait;
use futures_util::StreamExt;
use microbit::{
    display::nonblocking::{Display, Frame, MicrobitFrame},
    hal::{Clocks, Rtc},
    pac::TIMER0,
};
use microbit_text::image::BitImage;
use nrf52833_hal::gpio::{Floating, Input, Pin};
use nrf_time::{initialize_timer, Duration, Ticker, Timer};
use tiny_led_matrix::Render;

const MAX_DIGITS: usize = 5;
static DISPLAY: Mutex<RefCell<Option<Display<TIMER0>>>> = Mutex::new(RefCell::new(None));
static DIGIT_DISPLAY: Mutex<RefCell<DigitDisplay<MAX_DIGITS>>> =
    Mutex::new(RefCell::new(DigitDisplay::new()));
static SHOWING_TEXT: AtomicBool = AtomicBool::new(false);

enum State {
    SelectingMode,
}

#[derive(Clone, Copy, Debug, defmt::Format)]
enum Mode {
    CountDown,
    CountUp,
}

impl Render for Mode {
    fn brightness_at(&self, x: usize, y: usize) -> u8 {
        match self {
            Mode::CountDown => [
                [0, 0, 9, 0, 0],
                [0, 0, 9, 0, 0],
                [9, 0, 9, 0, 9],
                [0, 9, 9, 9, 0],
                [0, 0, 9, 0, 0],
            ][y][x],
            Mode::CountUp => [
                [0, 0, 9, 0, 0],
                [0, 9, 9, 9, 0],
                [9, 0, 9, 0, 9],
                [0, 0, 9, 0, 0],
                [0, 0, 9, 0, 0],
            ][y][x],
        }
    }
}

struct Blank;

impl Render for Blank {
    fn brightness_at(&self, _x: usize, _y: usize) -> u8 {
        0
    }
}

struct DigitDisplay<const N: usize> {
    digits: [u8; N],
    count: usize,
    current_digit: usize,
}

impl<const N: usize> DigitDisplay<N> {
    const fn new() -> Self {
        Self {
            digits: [0; N],
            count: 1,
            current_digit: N,
        }
    }

    fn set(&mut self, n: u64) {
        let mut n = n;
        let mut i = Self::num_digits(n);

        while n > 0 {
            i -= 1;
            self.digits[i] = (n % 10) as u8;
            n /= 10;
        }
        self.count = i;
        self.current_digit = i;
    }

    fn num_digits(n: u64) -> usize {
        if n == 0 {
            return 1;
        }
        let mut n = n;
        let mut i = 0;
        while n > 0 {
            n /= 10;
            i += 1;
        }
        i
    }

    fn show(&mut self) {
        self.current_digit = 0;
    }

    fn hide(&mut self) {
        self.current_digit = N;
    }

    fn is_visible(&self) -> bool {
        self.current_digit < N
    }

    fn render(&self) -> &'static BitImage {
        microbit_text::font::character(self.digits[self.current_digit] + b'0')
    }

    fn next(&mut self) -> bool {
        self.current_digit += 1;
        if self.current_digit >= self.count {
            self.current_digit = 0;
            true
        } else {
            false
        }
    }
}

fn draw<T>(image: &T)
where
    T: Render,
{
    let mut frame = MicrobitFrame::default();
    frame.set(image);
    cortex_m::interrupt::free(|cs| {
        let mut display = DISPLAY.borrow(cs).borrow_mut();
        if let Some(display) = display.as_mut() {
            display.show_frame(&frame);
        }
    });
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let mut board = microbit::Board::take().unwrap();
    Clocks::new(board.CLOCK).start_lfclk();

    let rtc = Rtc::new(board.RTC2, 0).unwrap();
    initialize_timer!(RTC2 from (rtc, &mut board.NVIC));

    cortex_m::interrupt::free(|cs| {
        DISPLAY
            .borrow(cs)
            .replace(Some(Display::new(board.TIMER0, board.display_pins)));
    });

    spawner.spawn(run_display()).unwrap();
    spawner.spawn(run_scroller()).unwrap();

    let mut btn_a = board.buttons.button_a.degrade();
    let mut btn_b = board.buttons.button_b.degrade();

    let mode = select_mode(&mut btn_a, &mut btn_b).await;
    match mode {
        Mode::CountDown => todo!("CountDown"),
        Mode::CountUp => count_up().await,
    }
}

async fn select_mode(btn_a: &mut Pin<Input<Floating>>, btn_b: &mut Pin<Input<Floating>>) -> Mode {
    let mut mode = Mode::CountDown;
    loop {
        draw(&mode);
        let btn = select(btn_a.wait_for_low(), btn_b.wait_for_low()).await;
        match btn {
            Either::First(_) => {
                mode = match mode {
                    Mode::CountDown => Mode::CountUp,
                    Mode::CountUp => Mode::CountDown,
                }
            }
            Either::Second(_) => break,
        }
        btn_a.wait_for_high().await.unwrap();
        Timer::after_millis(200).await;
    }
    btn_b.wait_for_high().await.unwrap();
    info!("Selected mode: {:?}", mode);
    draw(&Blank);
    Timer::after_millis(250).await;
    draw(&mode);
    Timer::after_millis(500).await;
    draw(&Blank);
    Timer::after_millis(250).await;
    mode
}

async fn count_down() {
    todo!()
}

async fn count_up() {
    let mut count = 0u64;
    SHOWING_TEXT.store(true, Ordering::Relaxed);
    loop {
        cortex_m::interrupt::free(|cs| {
            let mut digit_display = DIGIT_DISPLAY.borrow(cs).borrow_mut();
            digit_display.set(count);
            digit_display.show();
        });

        Timer::after_secs(60).await;
        count += 1;
    }
}

#[embassy_executor::task]
async fn run_display() {
    Ticker::every(Duration::from_micros(50))
        .for_each(|_| async {
            cortex_m::interrupt::free(|cs| {
                let mut display = DISPLAY.borrow(cs).borrow_mut();
                if let Some(display) = display.as_mut() {
                    display.handle_display_event();
                }
            });
        })
        .await;
}

#[embassy_executor::task]
async fn run_scroller() {
    Ticker::every(Duration::from_secs(5))
        .for_each(|_| async {
            if !SHOWING_TEXT.load(Ordering::Relaxed) {
                return;
            }
            #[derive(Eq, PartialEq)]
            enum Pause {
                None,
                Short,
                Long,
            }
            let mut pause = Pause::Short;
            while pause == Pause::Short {
                pause = cortex_m::interrupt::free(|cs| {
                    let mut digit_display = DIGIT_DISPLAY.borrow(cs).borrow_mut();
                    if !digit_display.is_visible() {
                        return Pause::None;
                    }
                    draw(digit_display.render());
                    if digit_display.next() {
                        Pause::Long
                    } else {
                        Pause::Short
                    }
                });

                let duration = match pause {
                    Pause::None => (0, 0),
                    Pause::Short => (500, 500),
                    Pause::Long => (5000, 500),
                };

                Timer::after_millis(duration.0).await;
                draw(&Blank);
                Timer::after_millis(duration.1).await;
            }
        })
        .await;
}
