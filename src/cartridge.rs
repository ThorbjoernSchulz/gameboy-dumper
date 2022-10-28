use crate::shift::{BitOrder, ShiftRegister};
use arduino_hal::port::{mode, Pin};

use core::mem;
use core::slice;

pub type InputPin = Pin<mode::Input<mode::Floating>>;
pub type OutputPin = Pin<mode::Output>;
pub type InputPins = [InputPin; 8];
pub type OutputPins = [OutputPin; 8];
pub type OutputBuffer = [u8; 512];

pub struct CartridgeConnection {
    pub address_in: ShiftRegister,
    pub read_pin: Pin<mode::Output>,
    pub write_pin: Pin<mode::Output>,
    pub input_pins: Option<InputPins>,
    pub output_pins: Option<OutputPins>,
    pub header: Option<CartridgeHeader>,
    pub mbc: MemoryBankController,
}

pub enum MemoryBankController {
    RomOnly,
    MBC1,
    MBC2,
    MBC3,
    MBC5,
}

impl MemoryBankController {
    pub fn from_cartridge_header(header: &CartridgeHeader) -> Self {
        match header.cartridge_type {
            0 => Self::RomOnly,
            0x01..=0x03 => Self::MBC1,
            0x05 | 0x06 => Self::MBC2,
            0x0F..=0x13 => Self::MBC3,
            0x19..=0x1E => Self::MBC5,
            _ => panic!(),
        }
    }
}

#[repr(C, packed)]
pub struct CartridgeHeader {
    pub entry_point: u32,
    pub nintendo_logo: [u8; 48],
    pub title: [u8; 16],
    pub licence_code: [u8; 2],
    pub sgb_flag: u8,
    pub cartridge_type: u8,
    pub rom_size: u8,
    pub ram_size: u8,
    pub destination_code: u8,
    pub old_license_code: u8,
    pub mask_rom_version: u8,
    pub header_checksum: u8,
    pub global_checksum: u16,
}

impl CartridgeHeader {
    /// returns the amound of rom banks
    pub fn decode_rom_size(&self) -> u16 {
        2 << self.rom_size
    }

    /// returns the amound of ram banks
    pub fn decode_ram_size(&self) -> u8 {
        match self.ram_size {
            0 | 1 => 0,
            2 => 1,
            3 => 4,
            4 => 16,
            5 => 8,
            _ => panic!(),
        }
    }

    pub fn serialize(&self) -> &[u8] {
        let p: *const Self = self;
        let p: *const u8 = p as *const u8;
        unsafe { slice::from_raw_parts(p, mem::size_of::<Self>()) }
    }

    pub fn from_cartridge_connection(cart: &mut CartridgeConnection) -> Self {
        let bytes = cart.read_block(0);

        let header: Self = unsafe { core::ptr::read(bytes.as_ptr().offset(0x100) as *const _) };
        header
    }
}

impl CartridgeConnection {
    pub fn new(
        address_in: ShiftRegister,
        read_pin: Pin<mode::Output>,
        write_pin: Pin<mode::Output>,
        data_pins: InputPins,
    ) -> Self {
        let mut ret = Self {
            address_in: address_in,
            read_pin: read_pin,
            write_pin: write_pin,
            input_pins: Some(data_pins),
            output_pins: None,
            header: None,
            mbc: MemoryBankController::RomOnly,
        };
        let header = CartridgeHeader::from_cartridge_connection(&mut ret);
        ret.mbc = MemoryBankController::from_cartridge_header(&header);
        ret.header = Some(header);
        ret
    }

    pub fn select_rom_bank(&mut self, bank: u16) {
        match self.mbc {
            MemoryBankController::RomOnly => todo!(),
            MemoryBankController::MBC1 => {
                self.write_byte(0x2fff, (bank & 0x1F) as u8);
                let additional_bits = (bank >> 8) & 3;
                if additional_bits != 0 {
                    self.write_byte(0x4000, additional_bits as u8);
                }
            }
            MemoryBankController::MBC2 => {
                self.write_byte(0x0100, (bank & 0xF) as u8);
            }
            MemoryBankController::MBC3 => {
                self.write_byte(0x2fff, (bank & 0x7F) as u8);
            }
            MemoryBankController::MBC5 => {
                self.write_byte(0x3000, ((bank >> 8) & 1) as u8);
                self.write_byte(0x2fff, bank as u8);
            }
        };
    }

    pub fn select_ram_bank(&mut self, bank: u8) {
        match self.mbc {
            MemoryBankController::RomOnly => unimplemented!(),
            MemoryBankController::MBC1 => {
                self.write_byte(0x4000, bank & 3);
            }
            MemoryBankController::MBC2 => unimplemented!(),
            MemoryBankController::MBC3 => {
                self.write_byte(0x4000, bank & 3);
            }
            MemoryBankController::MBC5 => {
                self.write_byte(0x4000, bank & 0xF);
            }
        }
    }

    pub fn enable_ram(&mut self) {
        self.write_byte(0, 0x0A);
    }

    pub fn disable_ram(&mut self) {
        self.write_byte(0, 0);
    }

    pub fn read_block(&mut self, address: u16) -> OutputBuffer {
        let mut bytes = [0u8; 512];
        let mut address = address;
        for b in &mut bytes {
            *b = self.read_byte(address);
            address += 1;
        }

        bytes
    }

    fn set_address(&mut self, address: u16) {
        self.address_in.latch_low();
        self.address_in
            .shift_out(address as u8, BitOrder::LstSigFirst);
        self.address_in
            .shift_out((address >> 8) as u8, BitOrder::LstSigFirst);
        self.address_in.latch_high();
    }

    fn read_byte(&mut self, address: u16) -> u8 {
        self.write_pin.set_high();
        self.read_pin.set_low();
        self.set_address(address);
        let value = data_pins_to_byte(self.input_pins.as_ref().unwrap());
        self.read_pin.set_high();
        value
    }

    pub fn write_byte(&mut self, address: u16, byte: u8) {
        self.data_pins_to_output();

        self.set_address(address);

        for i in 0..8 {
            if (byte & (1 << i)) != 0 {
                self.output_pins.as_mut().unwrap()[7 - i].set_high();
            } else {
                self.output_pins.as_mut().unwrap()[7 - i].set_low();
            }
        }

        arduino_hal::delay_ms(2);

        self.read_pin.set_high();
        self.write_pin.set_low();

        arduino_hal::delay_ms(2);

        self.write_pin.set_high();

        for pin in self.output_pins.as_mut().unwrap() {
            pin.set_low();
        }

        self.data_pins_to_input();
    }

    fn data_pins_to_input(&mut self) {
        let [d2, d3, d4, d5, d6, d7, d8, d9] = mem::replace(&mut self.output_pins, None).unwrap();

        self.input_pins = Some([
            d2.into_floating_input(),
            d3.into_floating_input(),
            d4.into_floating_input(),
            d5.into_floating_input(),
            d6.into_floating_input(),
            d7.into_floating_input(),
            d8.into_floating_input(),
            d9.into_floating_input(),
        ]);
    }
    fn data_pins_to_output(&mut self) {
        let [d2, d3, d4, d5, d6, d7, d8, d9] = mem::replace(&mut self.input_pins, None).unwrap();
        self.output_pins = Some([
            d2.into_output(),
            d3.into_output(),
            d4.into_output(),
            d5.into_output(),
            d6.into_output(),
            d7.into_output(),
            d8.into_output(),
            d9.into_output(),
        ]);
    }
}

/// Set address pins to specified address
fn data_pins_to_byte(pins: &[InputPin; 8]) -> u8 {
    let values = pins.iter().map(|p| p.is_high());
    let mut byte = 0u8;
    for v in values {
        byte <<= 1;
        byte |= v as u8;
    }
    byte
}
