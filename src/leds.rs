use embassy_stm32::spi::{Mode, Phase, Polarity};
use rgb::RGB8;

pub const MODE: Mode = Mode {
    polarity: Polarity::IdleLow,
    phase: Phase::CaptureOnFirstTransition,
};

pub struct Ws2812<SPI> {
    spi: SPI,
}

//

impl<SPI> Ws2812<SPI>
where
    SPI: embedded_hal::blocking::spi::Write<u8>,
{
    /// Use ws2812 devices via spi
    ///
    /// The SPI bus should run within 2 MHz to 3.8 MHz
    pub fn new(spi: SPI) -> Self {
        Self { spi }
    }

    /// Write a single byte for ws2812 devices
    async fn write_byte(&mut self, mut data: u8) -> Result<(), SPI::Error> {
        // Send two bits in one spi byte. High time first, then the low time
        // The maximum for T0H is 500ns, the minimum for one bit 1063 ns.
        // These result in the upper and lower spi frequency limits
        let patterns = [0b0100_0100, 0b0100_0111, 0b0111_0100, 0b0111_0111];
        for _ in 0..4 {
            let bits = (data & 0b1100_0000) >> 6;
            self.spi.write(&[patterns[bits as usize]])?;
            data <<= 2;
        }
        Ok(())
    }

    async fn flush(&mut self) -> Result<(), SPI::Error> {
        for _ in 0..20 {
            self.spi.write(&[0])?;
        }
        Ok(())
    }

    /// Write all the items of an iterator to a ws2812 strip
    pub async fn write<T, I>(&mut self, iterator: T) -> Result<(), SPI::Error>
    where
        T: Iterator<Item = I>,
        I: Into<RGB8>,
    {
        // We introduce an offset in the fifo here, so there's always one byte in transit
        // Some MCUs (like the stm32f1) only a one byte fifo, which would result
        // in overrun error if two bytes need to be stored
        // self.spi.write(0);
        if cfg!(feature = "mosi_idle_high") {
            self.flush().await?;
        }

        for item in iterator {
            let item: RGB8 = item.into();
            self.write_byte(item.g).await?;
            self.write_byte(item.r).await?;
            self.write_byte(item.b).await?;
        }
        self.flush().await?;
        // Now, resolve the offset we introduced at the beginning
        Ok(())
    }
}
