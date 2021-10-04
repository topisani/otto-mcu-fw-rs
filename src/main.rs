#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]
#![allow(incomplete_features)]
#![feature(generic_const_exprs)]
#![feature(generators, generator_trait)]

mod input;
mod keys;
mod util;

use defmt_rtt as _;
use embassy::executor::Spawner;
use embassy::time::{Duration, Timer};
use embassy_stm32::gpio::{Level, Output, Pin,  Speed};
use embassy_stm32::pac::AFIO;
use embassy_stm32::time::U32Ext;
use embassy_stm32::{Config, Peripherals};
use embedded_hal::digital::v2::OutputPin;
use keys::KeyMatrix;
// global logger
use panic_probe as _;

pub use defmt::*;

use core::sync::atomic::{AtomicUsize, Ordering};

defmt::timestamp! {
    "{=u64}", {
        static COUNT: AtomicUsize = AtomicUsize::new(0);
        // NOTE(no-CAS) `timestamps` runs with interrupts disabled
        let n = COUNT.load(Ordering::Relaxed);
        COUNT.store(n + 1, Ordering::Relaxed);
        n as u64
    }
}


fn config() -> Config {
    let mut config = Config::default();
    config.rcc.hse = Some(16.mhz().into());
    config.rcc.sys_ck = Some(48.mhz().into());
    config.rcc.hclk = Some(48.mhz().into());
    config.rcc.pclk1 = Some(24.mhz().into());
    config.rcc.pclk2 = Some(48.mhz().into());
    config.rcc.adcclk = Some(12.mhz().into());
    config
}

#[embassy::main(config = "config()")]
async fn main(spawner: Spawner, p: Peripherals) {
    // We use PB3 and PB4 for the keyboard matrix, so disable JTAG (keeping SWD enabled).
    unsafe {
        AFIO.mapr().modify(|m| m.set_swj_cfg(010u8));
    }
    let km = KeyMatrix::new(
        [
            p.PC9.degrade(),
            p.PB12.degrade(),
            p.PB13.degrade(),
            p.PB14.degrade(),
            p.PB15.degrade(),
            p.PB8.degrade(),
            p.PB4.degrade(),
            p.PB9.degrade(),
        ],
        [
            p.PB11.degrade(),
            p.PB0.degrade(),
            p.PA6.degrade(),
            p.PC4.degrade(),
            p.PC5.degrade(),
            p.PB10.degrade(),
            p.PB1.degrade(),
            p.PB3.degrade(),
        ],
    );

    spawner.spawn(input::poll_input(km)).unwrap();
    let mut led = Output::new(p.PC6, Level::High, Speed::Low);

    loop {
        unwrap!(led.set_high());
        Timer::after(Duration::from_millis(300)).await;

        unwrap!(led.set_low());
        Timer::after(Duration::from_millis(300)).await;
    }
}
