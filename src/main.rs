#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]
#![allow(incomplete_features)]
#![feature(generic_const_exprs)]
#![feature(generators, generator_trait)]

mod i2c;
mod input;
mod keys;
mod leds;
mod util;

use defmt::{info, unwrap};
use defmt_rtt as _;
use embassy::executor::Spawner;
use embassy::time::{Duration, Timer};
use embassy_stm32::dma::NoDma;
use embassy_stm32::gpio::{Level, Output, Pin, Speed};
use embassy_stm32::pac::AFIO;
use embassy_stm32::time::U32Ext;
use embassy_stm32::{interrupt, peripherals, spi, Config, Peripherals};
use embedded_hal::digital::v2::OutputPin;
use keys::KeyMatrix;
// global logger
use panic_probe as _;
use rgb::RGB8;

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

type Leds = leds::Ws2812<spi::Spi<'static, peripherals::SPI1, NoDma, NoDma>>;

#[embassy::task]
async fn test_leds(mut leds: Leds) {
    let mut colors = [RGB8::default(); 54];
    loop {
        for i in 0..colors.len() {
            colors[i] = RGB8::new(0xFF, 00, 0x20);
            leds.write(colors.clone().into_iter()).await.unwrap();
            Timer::after(Duration::from_millis(300)).await;
        }
        for i in 0..colors.len() {
            colors[i] = RGB8::new(0, 0, 0);
            leds.write(colors.clone().into_iter()).await.unwrap();
            Timer::after(Duration::from_millis(300)).await;
        }
    }
}

#[embassy::main(config = "config()")]
async fn main(spawner: Spawner, p: Peripherals) {
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
            p.PC1.degrade(), // Not right
            // p.PA6.degrade(),
            p.PC4.degrade(),
            p.PC5.degrade(),
            p.PB10.degrade(),
            p.PB1.degrade(),
            p.PB3.degrade(),
        ],
    );

    let mut spi_config = spi::Config::default();
    spi_config.mode = leds::MODE;
    let spi = spi::Spi::new(
        p.SPI1,
        p.PA5,
        p.PA7,
        p.PA6,
        NoDma,
        NoDma,
        3.mhz(),
        spi_config,
    );
    let leds = leds::Ws2812::new(spi);

    let i2c = i2c::I2cSlave::new(
        p.I2C1,
        p.PB6,
        p.PB7,
        interrupt::take!(I2C1_EV),
        interrupt::take!(I2C1_ER),
        0x77,
    );

    // We use PB3 and PB4 for the keyboard matrix, so disable JTAG (keeping SWD enabled).
    unsafe {
        AFIO.mapr().modify(|m| m.set_swj_cfg(010u8));
    }

    unwrap!(spawner.spawn(input::poll_input(km)));
    unwrap!(spawner.spawn(test_leds(leds)));
    let mut led = Output::new(p.PC6, Level::High, Speed::Low);

    loop {
        unwrap!(led.set_high());
        Timer::after(Duration::from_millis(300)).await;

        unwrap!(led.set_low());
        Timer::after(Duration::from_millis(300)).await;
    }
}
