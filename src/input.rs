use defmt::info;
use embassy::time::{Duration, Timer};
use embassy_stm32::gpio::{AnyPin, Pin};
use embassy_stm32::Peripherals;
use num_enum::TryFromPrimitive;

use crate::keys::KeyMatrix;

#[derive(Debug, Eq, PartialEq, TryFromPrimitive, defmt::Format)]
#[repr(u8)]
pub enum Key {
    None = 0,
    Channel0 = 1,
    Channel1 = 2,
    Channel2 = 3,
    Channel3 = 4,
    Channel4 = 5,
    Channel5 = 6,
    Channel6 = 7,
    Channel7 = 8,
    Channel8 = 9,
    Channel9 = 10,
    Seq0 = 11,
    Seq1 = 12,
    Seq2 = 13,
    Seq3 = 14,
    Seq4 = 15,
    Seq5 = 16,
    Seq6 = 17,
    Seq7 = 18,
    Seq8 = 19,
    Seq9 = 20,
    Seq10 = 21,
    Seq11 = 22,
    Seq12 = 23,
    Seq13 = 24,
    Seq14 = 25,
    Seq15 = 26,
    BlueEncClick = 27,
    GreenEncClick = 28,
    YellowEncClick = 29,
    RedEncClick = 30,
    Shift = 31,
    Sends = 32,
    Plus = 33,
    Mixer = 34,
    Minus = 35,
    Fx1 = 36,
    Fx2 = 37,
    Master = 38,
    Play = 39,
    Record = 40,
    Arp = 41,
    Slots = 42,
    Twist1 = 43,
    Twist2 = 44,
    Looper = 45,
    External = 46,
    Sampler = 47,
    Envelope = 48,
    Voices = 49,
    Settings = 50,
    Sequencer = 51,
    Synth = 52,
    UnassignedA = 53,
    UnassignedB = 54,
    UnassignedC = 55,
    UnassignedD = 56,
    UnassignedE = 57,
    UnassignedF = 58,
}

pub fn make_key_table() -> [[Key; 8]; 8] {
    [
        [
            Key::Seq0,
            Key::Channel2,
            Key::Channel5,
            Key::Channel8,
            Key::Twist1,
            Key::Sends,
            Key::BlueEncClick,
            Key::Sampler,
        ],
        [
            Key::Channel0,
            Key::Channel3,
            Key::Channel6,
            Key::Channel9,
            Key::Fx2,
            Key::Fx1,
            Key::YellowEncClick,
            Key::Looper,
        ],
        [
            Key::Channel1,
            Key::Channel4,
            Key::Channel7,
            Key::Seq15,
            Key::Mixer,
            Key::UnassignedC,
            Key::None,
            Key::Sequencer,
        ],
        [
            Key::Seq1,
            Key::Seq6,
            Key::Seq11,
            Key::UnassignedD,
            Key::Play,
            Key::Envelope,
            Key::RedEncClick,
            Key::Synth,
        ],
        [
            Key::Seq2,
            Key::Seq7,
            Key::Seq12,
            Key::UnassignedE,
            Key::Twist2,
            Key::UnassignedA,
            Key::None,
            Key::None,
        ],
        [
            Key::Seq3,
            Key::Seq8,
            Key::Seq13,
            Key::Slots,
            Key::Minus,
            Key::External,
            Key::None,
            Key::Arp,
        ],
        [
            Key::Seq4,
            Key::Seq9,
            Key::Seq14,
            Key::UnassignedF,
            Key::Record,
            Key::UnassignedB,
            Key::GreenEncClick,
            Key::Settings,
        ],
        [
            Key::Seq5,
            Key::Seq10,
            Key::None,
            Key::Shift,
            Key::Plus,
            Key::Voices,
            Key::None,
            Key::Master,
        ],
    ]
}

#[embassy::task]
pub async fn poll_input(mut matrix: KeyMatrix) {
    let table: [[Key; 8]; 8] = make_key_table();
    loop {
        Timer::after(Duration::from_millis(10)).await;
        let old_state = matrix.states.clone();
        let changed = matrix.scan().await;
        if !changed {
            continue;
        }
        for r in 0..8 {
            for c in 0..8 {
                if table[r][c] == Key::None {
                    continue;
                }
                let idx = KeyMatrix::idx_of(r, c);
                if old_state.get(idx) != matrix.states.get(idx) {
                    if matrix.states.get(idx) {
                        info!("Press {}", table[r][c])
                    } else {
                        info!("Release {}", table[r][c])
                    }
                }
            }
        }
    }
}
