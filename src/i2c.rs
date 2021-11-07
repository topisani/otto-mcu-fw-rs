use core::{
    marker::PhantomData,
    mem::MaybeUninit,
    task::{Context, Poll},
};

use cortex_m::peripheral::{scb::VectActive, SCB};
use defmt::{error, trace};
use embassy::{
    interrupt::{Interrupt, InterruptExt},
    util::Unborrow,
    waitqueue::WakerRegistration,
};
use embassy_hal_common::unborrow;
use embassy_stm32::{
    gpio::{AnyPin, Pin},
    i2c::{Instance, SclPin, SdaPin},
    pac::{self, gpio, i2c::vals},
    peripherals,
};
use futures::Future;

use crate::cmd::PacketData;

pub trait InstanceExt: Instance {
    type ErInterrupt: Interrupt;
}

pac::interrupts!(
 ($inst:ident, i2c, $block:ident, ER, $irq:ident) => {
 impl InstanceExt for peripherals::$inst {
 type ErInterrupt = crate::interrupt::$irq;
 }
 };
);

struct AfPin<'d, T: Pin> {
    pin: T,
    phantom: PhantomData<&'d mut T>,
}

impl<'d, T: Pin> AfPin<'d, T> {
    fn new(pin: T) -> Self {
        cortex_m::interrupt::free(|_| {
            let r = pin.block();
            let n = pin._pin() as usize;
            let crlh = if n < 8 { 0 } else { 1 };
            unsafe {
                r.cr(crlh).modify(|w| {
                    w.set_mode(n % 8, gpio::vals::Mode::OUTPUT50);
                    w.set_cnf(n % 8, gpio::vals::Cnf::ALTOPENDRAIN);
                });
            }
        });
        Self {
            pin,
            phantom: PhantomData::default(),
        }
    }
}

pub struct State<'d, T: InstanceExt>(MaybeUninit<StateInner<'d, T>>);
impl<'d, T: InstanceExt> State<'d, T> {
    pub fn new() -> Self {
        Self(MaybeUninit::uninit())
    }
}

pub struct I2cSlave<'d, T: InstanceExt> {
    inner: *mut StateInner<'d, T>,
    ev_irq: T::Interrupt,
    er_irq: T::ErInterrupt,
}

impl<'d, T: InstanceExt> Unpin for I2cSlave<'d, T> {}

impl<'d, T: InstanceExt> I2cSlave<'d, T> {
    pub fn new(
        state: &'d mut State<'d, T>,
        p: impl Unborrow<Target = T> + 'd,
        scl: impl Unborrow<Target = impl SclPin<T>> + 'd,
        sda: impl Unborrow<Target = impl SdaPin<T>> + 'd,
        ev_irq: T::Interrupt,
        er_irq: T::ErInterrupt,
        add: u16,
    ) -> Self
    where
        'd: 'static,
    {
        unsafe { Self::new_unchecked(state, p, scl, sda, ev_irq, er_irq, add) }
    }

    /// Safety: The instance must not be leaked (drop must be run), since otherwise, the interrupts will not be disabled.
    pub unsafe fn new_unchecked(
        state: &'d mut State<'d, T>,
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

        let scl = AfPin::new(scl);
        let sda = AfPin::new(sda);

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
                reg.set_add(add << 1);
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

        let state_ptr = state.0.as_mut_ptr();

        *state_ptr = StateInner {
            scl,
            sda,
            phantom: PhantomData::default(),
            stage: Stage::Waiting,
            tx_buffer: Default::default(),
            rx_waker: WakerRegistration::new(),
        };

        assert!(
            SCB::vect_active() == VectActive::ThreadMode,
            "Can only be created from thread mode"
        );
        ev_irq.disable();
        ev_irq.set_handler(|p| {
            let state = unsafe { &mut *(p as *mut StateInner<'d, T>) };
            state.on_event();
        });
        ev_irq.set_handler_context(state_ptr as *mut ());
        ev_irq.enable();

        er_irq.disable();
        er_irq.set_handler(|p| {
            let state = unsafe { &mut *(p as *mut StateInner<'d, T>) };
            state.on_error();
        });
        er_irq.set_handler_context(state_ptr as *mut ());
        er_irq.enable();

        Self {
            inner: state_ptr,
            ev_irq,
            er_irq,
        }
    }

    fn with_inner<R>(&mut self, f: impl FnOnce(&mut StateInner<'d, T>) -> R) -> R {
        self.ev_irq.disable();
        self.er_irq.disable();

        // Safety: interrupts are disabled, so no concurrent accesses are possible
        let state = unsafe { &mut *self.inner };
        let r = f(state);

        self.ev_irq.enable();
        self.er_irq.enable();
        r
    }

    /// Returns the packet as Err if the queue is full
    pub fn enqueue(&mut self, packet: PacketData) -> Result<(), PacketData> {
        self.with_inner(|s| s.tx_buffer.enqueue(packet))
    }

    pub fn receive_message(&mut self) -> Read<'_, Self> {
        Read { i2cslave: self }
    }

    pub fn poll_received(
        mut self: core::pin::Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<PacketData> {
        self.with_inner(|state| state.poll_received_packet(cx))
    }
}

// Could maybe be done simply by using PollFn instead of all of this, but lifetimes become complicated
pub struct Read<'a, R> {
    i2cslave: &'a mut R,
}

impl<'d, T: InstanceExt> Future for Read<'_, I2cSlave<'d, T>> {
    type Output = PacketData;

    fn poll(mut self: core::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        core::pin::Pin::new(&mut *self.i2cslave).poll_received(cx)
    }
}

#[derive(Clone)]
pub enum Stage {
    Waiting,
    Transmitting(PacketData, usize),
    Receiving(heapless::Vec<u8, 64>),
    ReceivedDataReady(PacketData),
}

// Size of TX buffer in number of packets
const TX_BUFFER_SIZE: usize = 16;

pub struct StateInner<'d, T: InstanceExt> {
    scl: AfPin<'d, AnyPin>,
    sda: AfPin<'d, AnyPin>,
    phantom: PhantomData<&'d mut T>,

    stage: Stage,
    tx_buffer: heapless::spsc::Queue<PacketData, TX_BUFFER_SIZE>,
    rx_waker: WakerRegistration,
}

impl<'d, T: InstanceExt> StateInner<'d, T> {
    fn on_event(&mut self) {
        let regs = T::regs();
        let sr1 = unsafe { regs.sr1().read() };

        // trace!("ev");
        if sr1.addr() {
            // trace!("addr");
            // clear addr by reading sr2 after reading sr1
            let sr2 = unsafe { regs.sr2().read() };
            if sr2.tra() {
                self.stage = Stage::Transmitting(Default::default(), 0);
            } else {
                self.stage = Stage::Receiving(heapless::Vec::new());
            }
        } else if let Stage::Transmitting(tx, idx) = self.stage {
            // trace!("tx");
            if sr1.tx_e() {
                // Transmit next byte
                unsafe { regs.dr().write(|dr| dr.set_dr(*tx.get(idx).unwrap_or(&0))) }
                self.stage = Stage::Transmitting(tx, idx + 1);
            }
        } else if let Stage::Receiving(rx_buf) = &mut self.stage {
            // trace!("rx");
            if sr1.rx_ne() {
                let byte = unsafe { regs.dr().read().dr() };
                if rx_buf.push(byte).is_err() {
                    error!("Error receiving message - message too long!");
                    // TODO: Reset
                }
            } else if sr1.stopf() {
                unsafe {
                    // Clear stopf by writing to cr1
                    regs.cr1().modify(|_| {});
                    // Disable ack until data has been read from buffer
                    regs.cr1().modify(|x| x.set_ack(false));
                }
                let packet = core::mem::take(rx_buf)
                    .into_array::<17>()
                    .unwrap_or_else(|e| {
                        error!("Received packet of length {}, expected 17", e.len());
                        Default::default()
                    });
                self.stage = Stage::ReceivedDataReady(packet);
                self.rx_waker.wake();
            }
        } else {
            trace!("Unknown error");
            if sr1.stopf() {
                unsafe {
                    // clear stopf
                    regs.sr1().read();
                    regs.cr1().modify(|_| {})
                }
            }

            if sr1.rx_ne() {
                // We're here because of an error, clear dr
                unsafe {
                    regs.dr().read();
                }
            }
        }
    }

    fn on_error(&mut self) {
        let regs = T::regs();
        let sr1 = unsafe { regs.sr1().read() };
        if let (Stage::Transmitting(..), true) = (&self.stage, sr1.af()) {
            // RM0008 fig 241: EV3-2
            // Was transmitting, got nack / stop condition.
            // If the whole packet was transmitted, pop it off
            error!("Was transmitting, got nack/stop");
            self.stage = Stage::Waiting;
            unsafe { regs.sr1().modify(|x| x.set_af(false)) };
        } else {
            error!(
                "Error: {}{}{}{}{}{}{}",
                if sr1.berr() { "BERR " } else { "" },
                if sr1.arlo() { "ARLO " } else { "" },
                if sr1.af() { "AF " } else { "" },
                if sr1.ovr() { "OVR " } else { "" },
                if sr1.pecerr() { "PECERR " } else { "" },
                if sr1.timeout() { "TIMEOUT " } else { "" },
                if sr1.smbalert() { "SMBALERT " } else { "" },
            );
            unsafe {
                regs.cr1().modify(|x| x.set_ack(true));
                regs.sr1().write_value(pac::i2c::regs::Sr1(0));
            }
            let sr1 = unsafe { regs.sr1().read() };
            // Clear flags by reading/modifying registers as specified
            if sr1.addr() {
                unsafe {
                    regs.sr1().read();
                    regs.sr2().read();
                }
            }
            if sr1.stopf() {
                unsafe {
                    regs.sr1().read();
                    regs.cr1().modify(|_| {});
                }
            }
            if sr1.rx_ne() {
                unsafe { regs.dr().read() };
            }
            self.stage = Stage::Waiting;
        }
    }

    fn poll_received_packet(&mut self, cx: &mut Context<'_>) -> Poll<PacketData> {
        if let Stage::ReceivedDataReady(x) = self.stage {
            self.stage = Stage::Waiting;
            unsafe { T::regs().cr1().modify(|x| x.set_ack(true)) }
            Poll::Ready(x)
        } else {
            self.rx_waker.register(cx.waker());
            Poll::Pending
        }
    }
}

impl<'d, T: InstanceExt> Drop for I2cSlave<'d, T> {
    fn drop(&mut self) {
        trace!("drop");
        T::disable();
    }
}
