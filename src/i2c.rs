use core::marker::PhantomData;

use embassy::{
    interrupt::{Interrupt, InterruptExt},
    util::Unborrow,
};
use embassy_hal_common::unborrow;
use embassy_stm32::{
    gpio::{AfType, AlternateFunctionPin, AnyPin, Pin},
    i2c::{Instance, SclPin, SdaPin},
    pac::{self, i2c::vals},
    peripherals,
};

trait InstanceExt: Instance {
    type ErInterrupt: Interrupt;
}

pac::interrupts!(
    ($inst:ident, i2c, $block:ident, ER, $irq:ident) => {
        impl InstanceExt for peripherals::$inst {
            type ErInterrupt = crate::interrupt::$irq;
        }
    };
);

pub struct I2cSlave<'d, T: Instance> {
    phantom: PhantomData<&'d mut T>,
    scl: AlternateFunctionPin<'d, AnyPin>,
    sda: AlternateFunctionPin<'d, AnyPin>,
}

impl<'d, T: InstanceExt> I2cSlave<'d, T> {
    pub fn new(
        p: impl Unborrow<Target = T> + 'd,
        scl: impl Unborrow<Target = impl SclPin<T>> + 'd,
        sda: impl Unborrow<Target = impl SdaPin<T>> + 'd,
        ev_irq: T::Interrupt,
        er_irq: T::ErInterrupt,
        add: u16,
    ) -> Self {
        unborrow!(scl, sda);

        let scl = scl.degrade();
        let sda = sda.degrade();

        T::enable();

        let scl = AlternateFunctionPin::new(scl, 0, AfType::OutputOpenDrain);
        let sda = AlternateFunctionPin::new(sda, 0, AfType::OutputOpenDrain);

        unsafe {
            T::regs().cr1().modify(|reg| {
                reg.set_pe(false);
            });
            T::regs().cr1().modify(|reg| {
                reg.set_engc(false);
                reg.set_nostretch(true);
            });
            T::regs().oar1().modify(|reg| {
                reg.set_addmode(vals::Addmode::ADD7);
                reg.set_add(add);
            });
            T::regs().oar2().modify(|reg| {
                reg.set_endual(vals::Endual::SINGLE);
            });
            T::regs().cr1().modify(|reg| {
                reg.set_pe(true);
            });
            T::regs().cr1().modify(|reg| {
                reg.set_pos(vals::Pos::CURRENT);
                reg.set_ack(true);
            });
            T::regs().cr2().modify(|reg| {
                reg.set_itbufen(true);
                reg.set_iterren(true);
                reg.set_itevten(true);
            });
        }

        ev_irq.set_handler(Self::on_ev_interrupt);
        ev_irq.unpend();
        ev_irq.enable();

        er_irq.set_handler(Self::on_er_interrupt);
        er_irq.unpend();
        er_irq.enable();

        Self {
            scl,
            sda,
            phantom: PhantomData::default(),
        }
    }

    fn on_ev_interrupt(_: *mut ()) {
        let regs = T::regs();
        let sr1 = unsafe { regs.sr1().read() };

        if sr1.addr() {
            // clear addr by reading sr2 after reading sr1
            let sr2 = unsafe { regs.sr2().read() };
            if sr2.tra() {
                // state = tx
                
            } else {
                // state = rx
            }
        } else if false
        /*transmitting*/
        {
            if sr1.tx_e() {
                // Transmit next byte
                unsafe {
                    regs.dr().write(|dr| dr.set_dr(0)); //todo
                }
            }
        } else if false
        /*receiving*/
        {
            if sr1.rx_ne() {
                let byte = unsafe { regs.dr().read().dr() };
            } else if sr1.stopf() {
                unsafe {
                    // Clear stopf by writing to cr1
                    regs.cr1().write(|_| {});
                    // Disable ack until data has been read from buffer
                    regs.cr1().modify(|x| x.set_ack(false));
                    // TODO: set state data ready
                }
            }
        } else {
            if sr1.stopf() {
                unsafe {
                    // clear stopf
                    regs.sr1().read();
                    regs.cr1().write(|_| {})
                }
            }

            if sr1.rx_ne() {
                // We're here because of an error, clear dr
                unsafe {
                    regs.dr().read();
                }
            }
        }

        // if isr.tcr() || isr.tc() {
        //     let n = T::state_number();
        //     STATE.chunks_transferred[n].fetch_add(1, Ordering::Relaxed);
        //     STATE.waker[n].wake();
        // }
    }

    fn on_er_interrupt(_: *mut ()) {}
}

impl<'d, T: Instance> Drop for I2cSlave<'d, T> {
    fn drop(&mut self) {
        T::disable();
    }
}
