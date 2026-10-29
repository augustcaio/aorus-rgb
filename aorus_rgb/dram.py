"""Memórias RGB como zonas da CLI (dram1, dram2, ...)."""

from dataclasses import dataclass

from . import ene
from .protocol import SPEED_MAX
from .smbus import SMBus

# efeitos da CLI (nomes do IT5702) -> modos equivalentes do ENE
EFFECT_TO_MODE = {
    "none": "off",
    "static": "static",
    "pulse": "breathing",
    "flash": "flashing",
    "cycle": "spectrum_cycle",
    "random": "random_flicker",
}


@dataclass(frozen=True)
class DramZone:
    name: str
    device: ene.EneDevice

    addressable = True


def ene_speed(speed):
    """Velocidade da CLI (0 rápido .. 9 lento) -> ENE (0 rápido .. 4 lento)."""
    if not 0 <= speed <= SPEED_MAX:
        raise ValueError(f"velocidade deve estar entre 0 e {SPEED_MAX}")
    return round(speed * ene.SPEED_SLOWEST / SPEED_MAX)


def rainbow_speed(speed):
    """Velocidade do `rainbow` da CLI (maior = mais rápido) -> ENE (0 rápido .. 4 lento)."""
    return min(ene.SPEED_SLOWEST, max(ene.SPEED_FASTEST, round(ene.SPEED_SLOWEST - speed * ene.SPEED_SLOWEST / 10)))


def _dim(color, brightness):
    if not 0 <= brightness <= 255:
        raise ValueError("brilho deve estar entre 0 e 255")
    return tuple(c * brightness // 255 for c in color)


class DramController:
    def __init__(self, bus=None):
        self.bus = bus or SMBus()
        self.zones = {f"dram{i}": DramZone(f"dram{i}", d) for i, d in enumerate(ene.discover(self.bus), 1)}
        self._direct = set()

    @property
    def path(self):
        return getattr(self.bus, "path", None)

    def close(self):
        self.bus.close()

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        self.close()

    def led_count(self, zone):
        return zone.device.led_count

    def set_colors(self, zone, colors):
        if zone.name not in self._direct:
            zone.device.set_direct(True)
            self._direct.add(zone.name)
        zone.device.set_colors(colors)

    def set_color(self, zone, color):
        self.set_colors(zone, [color] * self.led_count(zone))

    def set_effect(self, zone, effect, color, brightness=255, speed=5, apply=True):
        if effect not in EFFECT_TO_MODE:
            raise ValueError(f"efeito desconhecido: {effect}")
        colors = [_dim(color, brightness)] * self.led_count(zone)
        zone.device.set_mode(EFFECT_TO_MODE[effect], colors, ene_speed(speed))
        self._direct.discard(zone.name)

    def set_rainbow(self, zone, speed=5):
        """Rainbow animado pelo próprio chip, sem tráfego no SMBus depois de ligado."""
        zone.device.set_mode("rainbow", [(0, 0, 0)] * self.led_count(zone), rainbow_speed(speed))
        self._direct.discard(zone.name)

    def apply(self):
        """Cada set_mode já aplica no próprio chip."""

    def off(self, zone):
        self.set_effect(zone, "none", (0, 0, 0))
