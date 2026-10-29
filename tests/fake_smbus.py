"""SMBus simulado com controladores ENE de memória."""

from aorus_rgb import ene
from aorus_rgb.smbus import check_address

VERSION = b"AUDA0-E6K5-0101\0"


class FakeEne:
    def __init__(self, slot, led_count=5, version=VERSION, address=ene.DEFAULT_ADDRESS):
        self.slot = slot
        self.address = address
        self.pointer = 0
        self.regs = {ene.REG_DEVICE_NAME + i: b for i, b in enumerate(version.ljust(16, b"\0"))}
        self.regs[ene.REG_CONFIG_TABLE + ene.CONFIG_LED_COUNT] = led_count

    def write(self, value):
        self.regs[self.pointer] = value
        if self.pointer == ene.REG_I2C_ADDRESS and self.regs.get(ene.REG_SLOT_INDEX) == self.slot:
            self.address = value >> 1
        self.pointer += 1

    def colors(self, base, count):
        return [self.regs.get(base + i, 0) for i in range(count * 3)]


class FakeSMBus:
    """Chips no mesmo endereço respondem juntos, como no barramento real."""

    path = "/dev/i2c-fake"

    def __init__(self, chips=(), others=()):
        self.chips = list(chips)
        self.others = set(others)  # outros dispositivos que só respondem a leituras
        self.log = []
        self.block_supported = True

    def _at(self, address):
        check_address(address)
        return [c for c in self.chips if c.address == address]

    def read_byte(self, address):
        if not self._at(address) and address not in self.others:
            raise OSError(6, "No such device or address")
        return 0

    def read_byte_data(self, address, command):
        chips = self._at(address)
        if not chips:
            raise OSError(6, "No such device or address")
        if 0xA0 <= command <= 0xAF:
            return command - 0xA0
        if command == 0x81:
            value = 0xFF
            for c in chips:  # wired-AND
                value &= c.regs.get(c.pointer, 0)
            return value
        return 0

    def write_word_data(self, address, command, value):
        self.log.append(("word", address, command, value))
        for c in self._at(address):
            assert command == 0x00
            c.pointer = ((value << 8) & 0xFF00) | (value >> 8)

    def write_byte_data(self, address, command, value):
        self.log.append(("byte", address, command, value))
        for c in self._at(address):
            assert command == 0x01
            c.write(value)

    def write_block_data(self, address, command, values):
        if not self.block_supported:
            raise OSError(95, "Operation not supported")
        self.log.append(("block", address, command, list(values)))
        for c in self._at(address):
            assert command == 0x03
            for v in values:
                c.write(v)

    def writes(self):
        return [e for e in self.log if e[0] != "word"]

    def close(self):
        pass
