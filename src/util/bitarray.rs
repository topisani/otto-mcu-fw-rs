use defmt::Format;

#[derive(Default, Clone, PartialEq, Format)]
pub struct BitArray<const BITS: usize>
where
    [u8; (BITS + 7) / 8]: Default,
    [u8; (BITS + 7) / 8]: Format,
{
    bytes: [u8; (BITS + 7) / 8],
}

impl<const BITS: usize> BitArray<BITS>
where
    [u8; (BITS + 7) / 8]: Default,
    [u8; (BITS + 7) / 8]: Format,
{
    pub fn get(&self, idx: usize) -> bool {
        self.bytes[idx / 8] & (1 << (idx % 8)) != 0
    }
    pub fn set(&mut self, idx: usize, state: bool) {
        if state {
            self.bytes[idx / 8] |= (1 << (idx % 8)) as u8;
        } else {
            self.bytes[idx / 8] &= !((1 << (idx % 8)) as u8);
        }
    }
}
