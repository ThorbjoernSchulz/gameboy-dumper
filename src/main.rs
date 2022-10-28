#![no_std]
#![no_main]
use panic_halt as _;

mod cartridge;
mod shift;
use arduino_hal::pac::USART0;
use arduino_hal::port::{mode, Pin};
use cartridge::CartridgeConnection;
use shift::ShiftRegister;

enum Command {
    NoOp,
    DumpHeader,
    DumpRom,
    DumpRam,
    FlashRam,
}

impl Command {
    fn from_u8(value: u8) -> Command {
        match value {
            0 => Command::DumpHeader,
            1 => Command::DumpRom,
            2 => Command::DumpRam,
            4 => Command::FlashRam,
            _ => Command::NoOp,
        }
    }
}

type Serial = arduino_hal::hal::usart::Usart<
    USART0,
    Pin<mode::Input, arduino_hal::hal::port::PD0>,
    Pin<mode::Output, arduino_hal::hal::port::PD1>,
    arduino_hal::clock::MHz16,
>;

fn dump_rom_bank(bank: u16, cartridge: &mut CartridgeConnection, serial: &mut Serial) {
    let mut address_base = 0;
    if bank != 0 {
        address_base = 0x4000;
        cartridge.select_rom_bank(bank);
    }
    for i in 0..32 {
        let buffer = cartridge.read_block(address_base + i * 512);
        for b in buffer {
            serial.write_byte(b);
        }
    }
}

fn dump_ram_bank(bank: u8, cartridge: &mut CartridgeConnection, serial: &mut Serial) {
    let address_base = 0xA000;
    cartridge.select_ram_bank(bank);
    for i in 0..16 {
        let buffer = cartridge.read_block(address_base + i * 512);
        for b in buffer {
            serial.write_byte(b);
        }
    }
}

fn flash_ram_bank(bank: u8, cartridge: &mut CartridgeConnection, serial: &mut Serial) {
    cartridge.select_ram_bank(bank);
    for i in 0..256 {
        let mut buffer = [0u8; 32];
        for b in &mut buffer {
            *b = serial.read_byte();
        }
        for (j, b) in buffer.iter().enumerate() {
            cartridge.write_byte(0xA000 + i * 32 + j as u16, *b);
        }
        // send end of chunk
        serial.write_byte(0xAB);
    }
    // send end of bank
    serial.write_byte(0xAA);
}

#[arduino_hal::entry]
fn main() -> ! {
    let peripherals = arduino_hal::Peripherals::take().unwrap();
    let pins = arduino_hal::pins!(peripherals);

    let data_pins = [
        pins.d2.into_floating_input().downgrade(),
        pins.d3.into_floating_input().downgrade(),
        pins.d4.into_floating_input().downgrade(),
        pins.d5.into_floating_input().downgrade(),
        pins.d6.into_floating_input().downgrade(),
        pins.d7.into_floating_input().downgrade(),
        pins.d8.into_floating_input().downgrade(),
        pins.d9.into_floating_input().downgrade(),
    ];
    let shift_register = ShiftRegister {
        sdata_pin: pins.d10.into_output().downgrade(),
        latch_pin: pins.d11.into_output().downgrade(),
        clock_pin: pins.d12.into_output().downgrade(),
    };

    let mut cart = cartridge::CartridgeConnection::new(
        shift_register,
        pins.a5.into_output_high().downgrade(),
        pins.d13.into_output_high().downgrade(),
        data_pins,
    );

    let mut serial = arduino_hal::default_serial!(peripherals, pins, 500_000);

    loop {
        /* Simple serial protocol using 2 bytes: "0xCA", COMMAND
         * Read one byte from serial. If it's "0xCA", a command is being sent,
         * so we dispatch on the next COMMAND byte and act accordingly.
         */
        let b = serial.read_byte();
        if b != 0xCA {
            continue;
        }
        let cmd = Command::from_u8(serial.read_byte());

        match cmd {
            Command::DumpHeader => {
                if let Some(header) = cart.header.as_ref() {
                    for b in header.serialize() {
                        serial.write_byte(*b);
                    }
                }
            }
            Command::DumpRom => {
                let mut num_banks = 0;
                if let Some(header) = cart.header.as_ref() {
                    num_banks = header.decode_rom_size();
                }

                for i in 0..num_banks {
                    dump_rom_bank(i, &mut cart, &mut serial);
                }
            }
            Command::DumpRam => {
                let mut num_banks = 0;
                if let Some(header) = cart.header.as_ref() {
                    num_banks = header.decode_ram_size();
                }

                cart.enable_ram();
                for i in 0..num_banks {
                    dump_ram_bank(i, &mut cart, &mut serial);
                }
                cart.disable_ram();
            }
            Command::FlashRam => {
                let mut num_banks = 0;
                if let Some(header) = cart.header.as_ref() {
                    num_banks = header.decode_ram_size();
                }

                cart.enable_ram();
                for i in 0..num_banks {
                    flash_ram_bank(i, &mut cart, &mut serial);
                }
                cart.disable_ram();
            }
            Command::NoOp => (),
        }
    }
}
