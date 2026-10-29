"""Acesso ao ITE IT5702 via hidraw e operações por zona."""

import dataclasses
import fcntl
import glob
import json
import os
import time
from pathlib import Path

from . import protocol as p

VID, PID = 0x048D, 0x5702

_IOC_WRITE, _IOC_READ = 1, 2


def _hid_ioc(nr, size):
    return ((_IOC_READ | _IOC_WRITE) << 30) | (size << 16) | (ord("H") << 8) | nr


HIDIOCSFEATURE = _hid_ioc(0x06, p.PACKET_SIZE)
HIDIOCGFEATURE = _hid_ioc(0x07, p.PACKET_SIZE)


class DeviceNotFound(Exception):
    pass


def find_hidraw():
    """Procura o hidraw do IT5702 via sysfs."""
    want = f"HID_ID=0003:{VID:08X}:{PID:08X}"
    for uevent in sorted(glob.glob("/sys/class/hidraw/hidraw*/device/uevent")):
        with open(uevent) as f:
            if want in f.read().upper():
                return "/dev/" + uevent.split("/")[4]
    raise DeviceNotFound("controlador ITE 048d:5702 não encontrado")


class HidrawTransport:
    def __init__(self, path=None):
        self.path = path or find_hidraw()
        self.fd = os.open(self.path, os.O_RDWR)

    def send(self, packet):
        fcntl.ioctl(self.fd, HIDIOCSFEATURE, packet)

    def get(self):
        buf = bytearray(p.PACKET_SIZE)
        buf[0] = p.REPORT_ID
        fcntl.ioctl(self.fd, HIDIOCGFEATURE, buf)
        return bytes(buf)

    def close(self):
        os.close(self.fd)


def _boot_id():
    try:
        return Path("/proc/sys/kernel/random/boot_id").read_text().strip()
    except OSError:
        return None


class State:
    """Estado que o chip não informa, válido só para o boot atual.

    - builtin_off_mask: máscara do comando 0x32. Sem ela, cada execução da CLI
      começaria com a máscara zerada e religaria o efeito embutido dos outros
      headers.
    - board_initialized: se a placa já foi inicializada neste boot (ver
      Controller._initialize).

    Ao ligar o PC o chip volta ao padrão, então o arquivo é descartado.
    """

    def __init__(self, path=None):
        if path is None:
            base = os.environ.get("XDG_STATE_HOME") or Path.home() / ".local/state"
            path = Path(base) / "aorus-rgb" / "state.json"
        self.path = Path(path)
        self.builtin_off_mask = 0
        self.board_initialized = False
        try:
            data = json.loads(self.path.read_text())
            if data.get("boot_id") == _boot_id():
                self.builtin_off_mask = int(data.get("builtin_off_mask", 0))
                self.board_initialized = bool(data.get("board_initialized", False))
        except (OSError, ValueError):
            pass

    def save(self):
        self.path.parent.mkdir(parents=True, exist_ok=True)
        data = {
            "boot_id": _boot_id(),
            "builtin_off_mask": self.builtin_off_mask,
            "board_initialized": self.board_initialized,
        }
        self.path.write_text(json.dumps(data))


class MemoryState(State):
    """Estado que não toca o disco (testes)."""

    def __init__(self):
        self.builtin_off_mask = 0
        self.board_initialized = False

    def save(self):
        pass


class Controller:
    def __init__(self, transport=None, state=None):
        self.transport = transport or HidrawTransport()
        self.state = state if state is not None else State()
        self.transport.send(p.command(p.CMD_INFO, 0x00))
        self.info = p.parse_info(self.transport.get())
        if not self.state.board_initialized:
            self._initialize()

    def _initialize(self):
        """Limpa o estado deixado por outro software (ex.: RGB Fusion no Windows).

        Sem isso, os efeitos animados (pulse, cycle, wave, random) não
        acendiam os headers ARGB na B550M AORUS ELITE, enquanto static e
        flash funcionavam. Roda uma vez por boot, porque zerar os efeitos a
        cada comando desfaria as cores das outras zonas.
        """
        for packet in p.reset_packets():
            self.transport.send(packet)
        self.transport.send(p.beat_packet(False))
        self.state.board_initialized = True
        self.state.save()

    @property
    def path(self):
        return getattr(self.transport, "path", None)

    def close(self):
        self.transport.close()

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        self.close()

    # --- estado dos headers ARGB ----------------------------------------
    def _set_builtin(self, zone, enabled):
        """Liga/desliga o efeito embutido de um header ARGB."""
        mask = self.state.builtin_off_mask
        mask = mask & ~zone.builtin_bit if enabled else mask | zone.builtin_bit
        self.transport.send(p.builtin_effect_off_packet(mask))
        self.state.builtin_off_mask = mask
        self.state.save()
        time.sleep(0.05)

    def led_count(self, zone):
        return self.info.argb_counts[zone.cal_slot]

    def set_led_count(self, zone, count):
        counts = list(self.info.argb_counts)
        counts[zone.cal_slot] = p.LED_COUNT_STEPS[p.led_count_code(count)]
        if tuple(counts) == self.info.argb_counts:
            return
        self.transport.send(p.led_count_packet(counts))
        self.info = dataclasses.replace(self.info, argb_counts=tuple(counts))

    # --- operações por zona ---------------------------------------------
    def set_colors(self, zone, colors):
        """Cores individuais por LED (só zonas ARGB)."""
        if self.state.builtin_off_mask & zone.builtin_bit == 0:
            self._set_builtin(zone, False)
        for packet in p.argb_packets(zone, colors, self.info.calibration[zone.cal_slot]):
            self.transport.send(packet)

    def set_color(self, zone, color):
        """Cor fixa na zona inteira."""
        if zone.addressable:
            self.set_colors(zone, [color] * self.led_count(zone))
        else:
            self.set_effect(zone, "static", color)

    def set_effect(self, zone, effect, color, brightness=255, speed=5, apply=True):
        """Efeito de hardware (roda no chip, sem o PC enviar quadros)."""
        if zone.addressable:
            self._set_builtin(zone, True)
        self.transport.send(p.effect_packet(zone, effect, color, brightness, speed))
        if apply:
            self.apply()

    def apply(self):
        self.transport.send(p.apply_packet())

    def off(self, zone):
        if zone.addressable:
            self.set_colors(zone, [(0, 0, 0)] * self.led_count(zone))
        else:
            self.set_effect(zone, "none", (0, 0, 0))
