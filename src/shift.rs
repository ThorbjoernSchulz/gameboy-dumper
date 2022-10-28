use arduino_hal::port::{mode, Pin};

pub struct ShiftRegister {
    pub sdata_pin: Pin<mode::Output>,
    pub latch_pin: Pin<mode::Output>,
    pub clock_pin: Pin<mode::Output>,
}

#[derive(Copy, Clone)]
#[allow(dead_code)]
pub enum BitOrder {
    MstSigFirst,
    LstSigFirst,
}

impl ShiftRegister {
    pub fn shift_out(&mut self, byte: u8, bitorder: BitOrder) {
        for i in 0..8 {
            let bit = match bitorder {
                BitOrder::MstSigFirst => (byte >> (7 - i)) & 1,
                BitOrder::LstSigFirst => (byte >> i) & 1,
            };
            match bit {
                0 => self.sdata_pin.set_low(),
                _ => self.sdata_pin.set_high(),
            }

            self.clock_pin.set_low();
            self.clock_pin.set_high();
        }
    }

    pub fn latch_low(&mut self) {
        self.latch_pin.set_low();
    }

    pub fn latch_high(&mut self) {
        self.latch_pin.set_high();
    }
}
