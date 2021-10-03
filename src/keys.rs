use embassy::time::Duration;
use embassy::time::Timer;
use embassy_stm32::gpio;
use embassy_stm32::gpio::AnyPin;
use embedded_hal::digital::v2::InputPin;
use embedded_hal::digital::v2::OutputPin;

use crate::util::bitarray::BitArray;

type AnyOutputPin = gpio::Output<'static, gpio::AnyPin>;
type AnyInputPin = gpio::Input<'static, gpio::AnyPin>;

const ROWS: usize = 8;
const COLS: usize = 8;

pub struct KeyMatrix {
    pub row_pins: [AnyInputPin; ROWS],
    pub col_pins: [AnyOutputPin; COLS],
    pub states: BitArray<64>,
}

impl KeyMatrix {
    pub fn new(row_pins: [AnyPin; ROWS], col_pins: [AnyPin; COLS]) -> Self {
        Self {
            row_pins: row_pins.map(|x| gpio::Input::new(x, gpio::Pull::Down)),
            col_pins: col_pins.map(|x| gpio::Output::new(x, gpio::Level::Low, gpio::Speed::Low)),
            states: Default::default(),
        }
    }

    pub fn idx_of(r: usize, c: usize) -> usize {
        c * COLS + r
    }

    /// Update the table; returns true if changed
    pub async fn scan(&mut self) -> bool {
        let mut has_changed = false;
        for c in 0..COLS {
            self.col_pins[c].set_high().unwrap();
            Timer::after(Duration::from_millis(1)).await;
            for r in 0..ROWS {
                let cur = self.row_pins[r].is_high().unwrap();
                if self.states.get(Self::idx_of(r, c)) != cur {
                    has_changed = true;
                }
                self.states.set(Self::idx_of(r, c), cur);
            }
            self.col_pins[c].set_low().unwrap();
        }
        has_changed
    }
}
