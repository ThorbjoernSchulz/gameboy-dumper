import serial
import time
from pathlib import Path

from argparse import ArgumentParser

BAUDRATE = 500_000


class CartridgeHeader:
    def __init__(self, hdrdata):
        self.__data = hdrdata
        self.nintendo_logo = hdrdata[0x4:0x34]
        self.title = self.__extract_title(hdrdata[0x34:0x44])
        self.cart_type = hdrdata[0x47]
        self.rom_size = hdrdata[0x48]
        self.ram_size = hdrdata[0x49]
        self.hdr_cksm = hdrdata[0x4D]

    def valid(self):
        x = 0
        for b in self.__data[0x34:0x4D]:
            x = (x - b - 1) % 256
        return x == self.hdr_cksm

    def __extract_title(self, data):
        title_end = 15
        if b"\0" in data:
            title_end = bytearray(data).index(0)
        return data[:title_end].decode("ascii")

    def info(self):
        print("[*] Title: ", self.title, flush=True)
        print("[*] Cartridge Type: ", hex(self.cart_type), flush=True)


def dump(ser, title, num_blocks):
    out_file = Path(title)
    dump_ = b""
    for i in range(num_blocks):
        print("[*] Dumping block {}/{}".format(i + 1, num_blocks), end="\r", flush=True)
        block = ser.read(512)
        dump_ += block

    out_file.write_bytes(dump_)


def flash(ser, data, num_banks):
    for i in range(num_banks):
        bank = data[i * 0x2000 : (i + 1) * 0x2000]
        print("[*] Flashing bank {}/{}".format(i + 1, num_banks), end="\r", flush=True)
        # flash in 32 byte chunks
        for j in range(256):
            chunk = bank[j * 32 : (j + 1) * 32]
            written = ser.write(chunk)
            assert written == 32
            # waiting for answer to synchronize
            ser.read()
        # waiting for bank to be flashed
        ser.read()


def main():
    parser = ArgumentParser(
        description="Dumps the content of either ROM or RAM of a Gameboy"
        " or GBC cartridge. Is also able to flash RAM."
    )

    parser.add_argument("--dumprom", nargs="?", const="c")
    parser.add_argument("--dumpram", nargs="?", const="c")
    parser.add_argument("--flashram")
    args = parser.parse_args()

    with serial.Serial(port="/dev/ttyACM0", baudrate=BAUDRATE) as ser:
        print("[*] Waiting for connection to be established...")
        time.sleep(2)
        print("[*] Receiving header...")

        ser.write(b"\xCA\x00")
        header = CartridgeHeader(ser.read(80))

        if header.valid():
            print("[*] Received valid header.")
        else:
            pass

        header.info()

        if not args.dumprom and not args.dumpram and not args.flashram:
            exit(0)

        if args.dumprom:
            print("[*] Dumping rom...")
            # figuring out how much memory we need to dump
            num_blocks = 32 * (2 << header.rom_size)

            file_name = args.dumprom if args.dumprom != "c" else header.title + ".rom"

            ser.write(b"\xCA\x01")

            dump(ser, file_name, num_blocks)

            print()
            print("[*] Done!")

        if args.dumpram:
            print("[*] Dumping ram...")
            # figuring out how much memory we need to dump
            num_blocks = {2: 1, 3: 4, 4: 16, 5: 8}.get(header.ram_size, 0)
            num_blocks *= 16

            file_name = args.dumpram if args.dumpram != "c" else header.title + ".ram"

            ser.write(b"\xCA\x02")

            dump(ser, file_name, num_blocks)

            print()
            print("[*] Done!")

        if args.flashram:
            print("[*] Flashing ram...")
            # figuring out how much memory we need to flash
            num_blocks = {2: 1, 3: 4, 4: 16, 5: 8}.get(header.ram_size, 0)

            file_name = args.flashram
            data = Path(file_name).read_bytes()

            ser.write(b"\xCA\x04")
            time.sleep(0.2)

            flash(ser, data, num_blocks)

            print()
            print("[*] Done!")


if __name__ == "__main__":
    main()
