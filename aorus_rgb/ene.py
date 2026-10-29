"""Controlador RGB ENE das memórias (AUDA0-E6K5-0101) via SMBus.

Protocolo baseado no ENESMBusController do OpenRGB (GPL-2.0). Os
registradores do chip têm 16 bits: o número do registrador é escrito
(com os bytes invertidos) no comando 0x00 e o valor é lido em 0x81,
escrito em 0x01 ou, em bloco, em 0x03.

Todo pente liga no endereço 0x77. Com mais de um pente, todos respondem
juntos ali até serem remapeados, um por slot, para 0x70-0x76. O remapeamento
dura até o PC ser desligado.
"""

from dataclasses import dataclass

from .smbus import SMBus

DEFAULT_ADDRESS = 0x77
REMAP_ADDRESSES = tuple(range(0x70, 0x77))
SLOTS = 8

REG_DEVICE_NAME = 0x1000
REG_CONFIG_TABLE = 0x1C00
REG_DIRECT = 0x8020
REG_MODE = 0x8021
REG_SPEED = 0x8022
REG_DIRECTION = 0x8023
REG_APPLY = 0x80A0
REG_SLOT_INDEX = 0x80F8
REG_I2C_ADDRESS = 0x80F9

CONFIG_LED_COUNT = 0x02
APPLY_VAL = 0x01
BLOCK_SIZE = 3

# versão do chip -> (registrador de cores diretas, de cores de efeito)
SUPPORTED = {"AUDA0-E6K5-0101": (0x8100, 0x8160)}

MODES = {
    "off": 0,
    "static": 1,
    "breathing": 2,
    "flashing": 3,
    "spectrum_cycle": 4,
    "rainbow": 5,
    "spectrum_cycle_breathing": 6,
    "chase_fade": 7,
    "spectrum_cycle_chase_fade": 8,
    "chase": 9,
    "spectrum_cycle_chase": 10,
    "spectrum_cycle_wave": 11,
    "chase_rainbow_pulse": 12,
    "random_flicker": 13,
}

SPEED_FASTEST, SPEED_SLOWEST = 0, 4


class UnsupportedDevice(Exception):
    pass


def _pointer(reg):
    return ((reg << 8) & 0xFF00) | (reg >> 8)


def present(bus, address):
    try:
        bus.read_byte(address)
        return True
    except OSError:
        return False


def is_ene(bus, address):
    """Assinatura ENE: registradores 0xA0-0xAF devolvem 0..15 (só leituras)."""
    try:
        return all(bus.read_byte_data(address, 0xA0 + i) == i for i in range(16))
    except OSError:
        return False


def remap(bus):
    """Dá a cada pente em 0x77 um endereço próprio. Devolve os endereços usados."""
    if not is_ene(bus, DEFAULT_ADDRESS):
        return []
    free = [a for a in REMAP_ADDRESSES if not present(bus, a)]
    used = []
    for slot in range(SLOTS):
        if not free or not present(bus, DEFAULT_ADDRESS):
            break
        address = free[0]
        _write(bus, DEFAULT_ADDRESS, REG_SLOT_INDEX, slot)
        _write(bus, DEFAULT_ADDRESS, REG_I2C_ADDRESS, address << 1)
        if present(bus, address):
            free.pop(0)
            used.append(address)
    return used


def _read(bus, address, reg):
    bus.write_word_data(address, 0x00, _pointer(reg))
    return bus.read_byte_data(address, 0x81)


def _write(bus, address, reg, value):
    bus.write_word_data(address, 0x00, _pointer(reg))
    bus.write_byte_data(address, 0x01, value)


@dataclass
class EneDevice:
    bus: SMBus
    address: int
    version: str
    led_count: int

    @classmethod
    def open(cls, bus, address):
        name = bytes(_read(bus, address, REG_DEVICE_NAME + i) for i in range(16))
        version = name.split(b"\0")[0].decode("ascii", "replace")
        if version not in SUPPORTED:
            raise UnsupportedDevice(f"controlador ENE {version!r} em {address:#04x} não suportado")
        led_count = _read(bus, address, REG_CONFIG_TABLE + CONFIG_LED_COUNT)
        return cls(bus, address, version, led_count)

    def read(self, reg):
        return _read(self.bus, self.address, reg)

    def write(self, reg, value):
        _write(self.bus, self.address, reg, value)

    def write_block(self, reg, values):
        self.bus.write_word_data(self.address, 0x00, _pointer(reg))
        try:
            self.bus.write_block_data(self.address, 0x03, values)
        except OSError:
            for v in values:
                self.bus.write_byte_data(self.address, 0x01, v)

    def _write_colors(self, reg, colors):
        if len(colors) != self.led_count:
            raise ValueError(f"esperava {self.led_count} cores, recebeu {len(colors)}")
        data = [c for r, g, b in colors for c in (r, b, g)]  # ordem do chip: R, B, G
        for i in range(0, len(data), BLOCK_SIZE):
            self.write_block(reg + i, data[i:i + BLOCK_SIZE])

    def set_direct(self, enabled):
        self.write(REG_DIRECT, 1 if enabled else 0)
        self.write(REG_APPLY, APPLY_VAL)

    def set_colors(self, colors):
        """Cores por LED em modo direto (o PC controla as cores)."""
        self._write_colors(SUPPORTED[self.version][0], colors)

    def set_mode(self, mode, colors, speed=2, direction=0):
        """Efeito de hardware, rodando no próprio chip."""
        if mode not in MODES:
            raise ValueError(f"modo desconhecido: {mode}")
        if not SPEED_FASTEST <= speed <= SPEED_SLOWEST:
            raise ValueError(f"velocidade ENE deve estar entre {SPEED_FASTEST} e {SPEED_SLOWEST}")
        self._write_colors(SUPPORTED[self.version][1], colors)
        self.write(REG_APPLY, APPLY_VAL)
        self.set_direct(False)
        self.write(REG_MODE, MODES[mode])
        self.write(REG_SPEED, speed)
        self.write(REG_DIRECTION, direction)
        self.write(REG_APPLY, APPLY_VAL)


def discover(bus):
    """Remapeia se preciso e devolve os pentes suportados, por endereço."""
    remap(bus)
    devices = []
    for address in (*REMAP_ADDRESSES, DEFAULT_ADDRESS):
        if present(bus, address) and is_ene(bus, address):
            try:
                devices.append(EneDevice.open(bus, address))
            except UnsupportedDevice:
                continue
    return devices
