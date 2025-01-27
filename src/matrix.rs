use core::ops::DerefMut;
use defmt::info;
use embassy_futures::join::join;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::Mutex;
use embedded_hal::digital::OutputPin;
use embedded_hal_async::delay::DelayNs;
use nrf52833_hal::{
    self as hal,
    gpio::{
        p0::{P0_11, P0_15, P0_19, P0_21, P0_22, P0_24, P0_28, P0_30, P0_31},
        p1::P1_05,
        Output, PushPull,
    },
    Timer,
};

pub struct Matrix(Mutex<CriticalSectionRawMutex, MutMatrix>);

impl Matrix {
    pub fn new(
        row1: P0_21<Output<PushPull>>,
        row2: P0_22<Output<PushPull>>,
        row3: P0_15<Output<PushPull>>,
        row4: P0_24<Output<PushPull>>,
        row5: P0_19<Output<PushPull>>,

        col1: P0_28<Output<PushPull>>,
        col2: P0_11<Output<PushPull>>,
        col3: P0_31<Output<PushPull>>,
        col4: P1_05<Output<PushPull>>,
        col5: P0_30<Output<PushPull>>,
    ) -> Self {
        Self(Mutex::new(MutMatrix::new(
            row1, row2, row3, row4, row5, col1, col2, col3, col4, col5,
        )))
    }

    pub async fn set(&self, row: u8, col: u8, value: bool) {
        let mut guard = self.0.lock().await;
        guard.set(row, col, value).await;
    }

    pub async fn clear(&self) {
        let mut guard = self.0.lock().await;
        guard.clear().await;
    }

    pub async fn clear_col(&self, col: u8) {
        let mut guard = self.0.lock().await;
        guard.deref_mut().clear_col(col).await;
    }

    pub async fn run<T: hal::timer::Instance>(&self, mut timer: Timer<T>) {
        loop {
            join(
                timer.delay_ms(20),
                async {
                    let mut guard = self.0.lock().await;
                    guard.deref_mut().draw().await;
                },
            ).await;
        }
    }
}

struct MutMatrix {
    row1: P0_21<Output<PushPull>>,
    row2: P0_22<Output<PushPull>>,
    row3: P0_15<Output<PushPull>>,
    row4: P0_24<Output<PushPull>>,
    row5: P0_19<Output<PushPull>>,

    col1: P0_28<Output<PushPull>>,
    col2: P0_11<Output<PushPull>>,
    col3: P0_31<Output<PushPull>>,
    col4: P1_05<Output<PushPull>>,
    col5: P0_30<Output<PushPull>>,

    display: [[bool; 5]; 5],
    current_row: u8,
}

impl MutMatrix {
    pub fn new(
        row1: P0_21<Output<PushPull>>,
        row2: P0_22<Output<PushPull>>,
        row3: P0_15<Output<PushPull>>,
        row4: P0_24<Output<PushPull>>,
        row5: P0_19<Output<PushPull>>,

        col1: P0_28<Output<PushPull>>,
        col2: P0_11<Output<PushPull>>,
        col3: P0_31<Output<PushPull>>,
        col4: P1_05<Output<PushPull>>,
        col5: P0_30<Output<PushPull>>,
    ) -> Self {
        Self {
            row1,
            row2,
            row3,
            row4,
            row5,

            col1,
            col2,
            col3,
            col4,
            col5,

            display: [[false; 5]; 5],
            current_row: 0,
        }
    }

    pub async fn set(&mut self, row: u8, col: u8, value: bool) {
        self.display[row as usize][col as usize] = value;
    }

    pub async fn clear(&mut self) {
        for row in self.display.iter_mut() {
            for col in row.iter_mut() {
                *col = false;
            }
        }
    }

    pub async fn clear_col(&mut self, col: u8) {
        for row in self.display.iter_mut() {
            row[col as usize] = false;
        }
    }

    async fn draw(&mut self) {
        self.clear_row(self.current_row).await;
        self.current_row = (self.current_row + 1) % 5;
        // Set the columns
        let row = self.display[self.current_row as usize];
        for (i, &col) in row.iter().enumerate() {
            self.write_col(i, !col).await;
        }
        self.set_row(self.current_row).await;
    }

    async fn set_row(&mut self, row: u8) {
        match row {
            0 => self.row1.set_high().unwrap(),
            1 => self.row2.set_high().unwrap(),
            2 => self.row3.set_high().unwrap(),
            3 => self.row4.set_high().unwrap(),
            4 => self.row5.set_high().unwrap(),
            n => panic!("Invalid row: {}", n),
        }
    }

    async fn clear_row(&mut self, row: u8) {
        match row {
            0 => self.row1.set_low().unwrap(),
            1 => self.row2.set_low().unwrap(),
            2 => self.row3.set_low().unwrap(),
            3 => self.row4.set_low().unwrap(),
            4 => self.row5.set_low().unwrap(),
            n => panic!("Invalid row: {}", n),
        }
    }

    async fn write_col(&mut self, col: usize, value: bool) {
        match col {
            0 if value => self.col1.set_high().unwrap(),
            1 if value => self.col2.set_high().unwrap(),
            2 if value => self.col3.set_high().unwrap(),
            3 if value => self.col4.set_high().unwrap(),
            4 if value => self.col5.set_high().unwrap(),

            0 if !value => self.col1.set_low().unwrap(),
            1 if !value => self.col2.set_low().unwrap(),
            2 if !value => self.col3.set_low().unwrap(),
            3 if !value => self.col4.set_low().unwrap(),
            4 if !value => self.col5.set_low().unwrap(),

            n => panic!("Invalid col: {}", n),
        }
    }
}
