"""Acesso ao SMBus via /dev/i2c-N (ioctl I2C_SMBUS), sem dependências.

O barramento das memórias também contém a EEPROM SPD, que guarda os
timings usados pelo BIOS no boot. Uma escrita errada ali pode impedir a
máquina de ligar, então esta camada recusa qualquer acesso a esses
endereços, independentemente de quem a chama.
"""

import ctypes
import fcntl
import glob
import os

I2C_SLAVE = 0x0703
I2C_SMBUS = 0x0720

_READ, _WRITE = 1, 0
_BYTE, _BYTE_DATA, _WORD_DATA, _BLOCK_DATA = 1, 2, 3, 5
BLOCK_MAX = 32

# 0x50-0x57: EEPROM SPD. 0x30-0x37: comandos de proteção/página da SPD DDR4,
# acionados por qualquer acesso, inclusive leitura.
FORBIDDEN = frozenset(range(0x30, 0x38)) | frozenset(range(0x50, 0x58))

DRAM_ADAPTER = "SMBus PIIX4 adapter port 0"


class BusNotFound(Exception):
    pass


class ForbiddenAddress(Exception):
    pass


class _Data(ctypes.Union):
    _fields_ = [
        ("byte", ctypes.c_uint8),
        ("word", ctypes.c_uint16),
        ("block", ctypes.c_uint8 * (BLOCK_MAX + 2)),
    ]


class _IoctlData(ctypes.Structure):
    _fields_ = [
        ("read_write", ctypes.c_uint8),
        ("command", ctypes.c_uint8),
        ("size", ctypes.c_uint32),
        ("data", ctypes.POINTER(_Data)),
    ]


def find_dram_bus():
    """/dev/i2c-N do SMBus da AMD onde ficam os slots de memória."""
    for name_file in sorted(glob.glob("/sys/class/i2c-dev/i2c-*/name")):
        with open(name_file) as f:
            if f.read().startswith(DRAM_ADAPTER):
                return "/dev/" + name_file.split("/")[4]
    raise BusNotFound(
        "SMBus das memórias não encontrado (falta acpi_enforce_resources=lax ou o módulo i2c-dev?)"
    )


def check_address(address):
    if address in FORBIDDEN:
        raise ForbiddenAddress(f"acesso ao endereço {address:#04x} bloqueado (SPD da memória)")
    if not 0x03 <= address <= 0x77:
        raise ValueError(f"endereço I2C inválido: {address:#04x}")


class SMBus:
    def __init__(self, path=None):
        self.path = path or find_dram_bus()
        self.fd = os.open(self.path, os.O_RDWR)
        self._address = None

    def close(self):
        os.close(self.fd)

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        self.close()

    def _access(self, address, read_write, command, size, data=None):
        check_address(address)
        if address != self._address:
            fcntl.ioctl(self.fd, I2C_SLAVE, address)
            self._address = address
        data = data or _Data()
        args = _IoctlData(read_write, command, size, ctypes.pointer(data))
        fcntl.ioctl(self.fd, I2C_SMBUS, args)
        return data

    def read_byte(self, address):
        """Lê um byte sem comando; OSError se ninguém responde."""
        return self._access(address, _READ, 0, _BYTE).byte

    def read_byte_data(self, address, command):
        return self._access(address, _READ, command, _BYTE_DATA).byte

    def write_byte_data(self, address, command, value):
        data = _Data()
        data.byte = value
        self._access(address, _WRITE, command, _BYTE_DATA, data)

    def write_word_data(self, address, command, value):
        data = _Data()
        data.word = value
        self._access(address, _WRITE, command, _WORD_DATA, data)

    def write_block_data(self, address, command, values):
        if len(values) > BLOCK_MAX:
            raise ValueError(f"bloco SMBus tem no máximo {BLOCK_MAX} bytes")
        data = _Data()
        data.block[0] = len(values)
        for i, v in enumerate(values, 1):
            data.block[i] = v
        self._access(address, _WRITE, command, _BLOCK_DATA, data)
