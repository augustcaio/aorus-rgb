"""Estado e animação de todas as zonas, sem D-Bus (usado pelo daemon).

O motor é o único dono dos dispositivos: aplica cor, efeito, arco-íris ou
desligado em cada zona, anima o arco-íris quadro a quadro e guarda a cena na
configuração para restaurá-la no próximo boot.
"""

import time

from . import protocol as p
from .config import Config
from .patterns import (
    DRAM_FRAME_DIVIDER,
    RAINBOW_FPS,
    light_first,
    palette_colors,
    rainbow_frame,
)

# mesma velocidade de fase do `rainbow` da CLI (speed / 1000 por quadro a 30 fps)
PHASE_PER_SECOND = RAINBOW_FPS / 1000
CALIBRATE_MAX = p.LED_COUNT_STEPS[2]
EFFECT_NAMES = tuple(e for e in p.EFFECTS if e != "none")
MODES = ("color", "effect", "rainbow", "off")
DEFAULT_RAINBOW = {"mode": "rainbow", "speed": 5.0, "brightness": 255}


def parse_hex(value):
    s = value.lstrip("#")
    try:
        if len(s) != 6:
            raise ValueError
        return tuple(int(s[i:i + 2], 16) for i in (0, 2, 4))
    except ValueError:
        raise ValueError(f"cor inválida: {value!r} (use RRGGBB)") from None


def _check_range(name, value, low, high):
    if not low <= value <= high:
        raise ValueError(f"{name} deve estar entre {low} e {high}")


class Engine:
    def __init__(self, board, dram=None, config=None, clock=time.monotonic):
        self.board = board
        self.dram = dram
        self.config = config if config is not None else Config()
        self.clock = clock
        self.t0 = clock()
        self.frame = 0
        self.zones = {name: (board, z) for name, z in p.ZONES.items()}
        if dram is not None:
            self.zones.update({name: (dram, z) for name, z in dram.zones.items()})
        self.scene = {}
        self.calibrating = set()
        self.listeners = []

    # --- consulta ------------------------------------------------------------
    def kind(self, name):
        ctl, zone = self.zones[name]
        if ctl is self.dram:
            return "dram"
        return "argb" if zone.addressable else "rgb"

    def zone_info(self):
        """(nome, tipo, LEDs) de cada zona, na ordem da placa e depois das memórias.

        Zonas RGB de 12 V têm uma cor só: contam como 1 LED.
        """
        info = []
        for name, (ctl, zone) in self.zones.items():
            kind = self.kind(name)
            info.append((name, kind, 1 if kind == "rgb" else ctl.led_count(zone)))
        return info

    def device_info(self):
        info = self.board.info
        result = {
            "board.product": info.product,
            "board.firmware": info.firmware,
            "board.chip_id": f"{info.chip_id:#010x}",
            "board.path": str(getattr(self.board, "path", "")),
        }
        if self.dram is not None:
            result["dram.path"] = str(getattr(self.dram, "path", ""))
            for name, z in self.dram.zones.items():
                result[f"{name}.controller"] = f"{z.device.version} em {z.device.address:#04x}"
        return result

    def resolve(self, name):
        """Nome de zona ou grupo (`all`, `dram`) -> nomes de zonas."""
        if name == "all":
            return list(self.zones)
        if name == "dram":
            return [n for n in self.zones if self.kind(n) == "dram"]
        if name not in self.zones:
            raise ValueError(f"zona desconhecida: {name}")
        return [name]

    @property
    def animated(self):
        return [
            n for n, spec in self.scene.items()
            if spec["mode"] == "rainbow" and spec["speed"] > 0 and n not in self.calibrating
        ]

    # --- comandos -------------------------------------------------------------
    def set_color(self, name, color):
        parse_hex(color)
        self._apply_many(self.resolve(name), {"mode": "color", "color": color.lstrip("#").lower()})

    def set_effect(self, name, effect, color, brightness=255, speed=5):
        if effect not in EFFECT_NAMES:
            raise ValueError(f"efeito desconhecido: {effect} (use {', '.join(EFFECT_NAMES)})")
        parse_hex(color)
        _check_range("brilho", brightness, 0, 255)
        _check_range("velocidade", speed, 0, p.SPEED_MAX)
        spec = {"mode": "effect", "effect": effect, "color": color.lstrip("#").lower(),
                "brightness": int(brightness), "speed": int(speed)}
        self._apply_many(self.resolve(name), spec)

    def set_rainbow(self, name, speed=5.0, brightness=255):
        _check_range("velocidade", speed, 0, 100)
        _check_range("brilho", brightness, 0, 255)
        names = [n for n in self.resolve(name) if self.kind(n) != "rgb"]
        if not names:
            raise ValueError(f"{name} não é endereçável: o arco-íris precisa de LEDs individuais")
        self._apply_many(names, {"mode": "rainbow", "speed": float(speed), "brightness": int(brightness)})

    def off(self, name):
        self._apply_many(self.resolve(name), {"mode": "off"})

    def apply_scene(self, scene):
        """Aplica uma cena (zona -> spec), ignorando zonas que não existem mais."""
        for name, spec in scene.items():
            if name in self.zones and spec.get("mode") in MODES:
                self._apply(name, dict(spec))
        self._changed()

    def restore(self):
        """Cena salva, ou arco-íris nas zonas endereçáveis na primeira execução."""
        scene = self.config.scene()
        if scene is None:
            scene = {n: dict(DEFAULT_RAINBOW) for n in self.zones if self.kind(n) != "rgb"}
        self.apply_scene(scene)

    # --- perfis ---------------------------------------------------------------
    def list_profiles(self):
        return sorted(self.config.profiles())

    def save_profile(self, name):
        name = name.strip()
        if not name or len(name) > 64:
            raise ValueError("o nome do perfil deve ter de 1 a 64 caracteres")
        self.config.save_profile(name, {n: dict(s) for n, s in self.scene.items()})
        self._notify()

    def apply_profile(self, name):
        profiles = self.config.profiles()
        if name not in profiles:
            raise ValueError(f"perfil não encontrado: {name}")
        self.apply_scene(profiles[name])

    def delete_profile(self, name):
        self.config.delete_profile(name)
        self._notify()

    # --- calibração dos headers ARGB -----------------------------------------
    def _argb(self, name):
        if name not in self.zones or self.kind(name) != "argb":
            raise ValueError(f"{name} não é um header ARGB da placa")
        return self.zones[name][1]

    def light_first(self, name, count):
        """Uma cor da paleta por LED nos primeiros `count` LEDs; o resto apagado."""
        zone = self._argb(name)
        _check_range("quantidade de LEDs", count, 1, CALIBRATE_MAX)
        self.calibrating.add(name)
        light_first(self.board, zone, count, palette_colors(count))

    def set_led_count(self, name, count):
        """Salva a calibração e volta a zona ao estado que tinha."""
        zone = self._argb(name)
        _check_range("quantidade de LEDs", count, 1, CALIBRATE_MAX)
        self.board.set_chip_led_count(zone, count)
        self.config.set_leds(name, count)
        self.end_calibration(name)

    def end_calibration(self, name):
        self._argb(name)
        self.calibrating.discard(name)
        if name in self.scene:
            self._apply(name, self.scene[name])
        self._notify()

    # --- animação ---------------------------------------------------------------
    def phase(self, speed, now=None):
        now = self.clock() if now is None else now
        return ((now - self.t0) * speed * PHASE_PER_SECOND) % 1

    def tick(self):
        """Um quadro do arco-íris animado; as memórias recebem 1 a cada N quadros."""
        now = self.clock()
        for name in self.animated:
            if self.kind(name) == "dram" and self.frame % DRAM_FRAME_DIVIDER:
                continue
            self._draw_rainbow(name, self.phase(self.scene[name]["speed"], now))
        self.frame += 1

    # --- interno ----------------------------------------------------------------
    def _draw_rainbow(self, name, phase):
        ctl, zone = self.zones[name]
        spec = self.scene[name]
        ctl.set_colors(zone, rainbow_frame(ctl.led_count(zone), phase, spec["brightness"] / 255))

    def _apply(self, name, spec):
        ctl, zone = self.zones[name]
        self.calibrating.discard(name)  # um comando novo encerra a calibração da zona
        mode = spec["mode"]
        if mode == "color":
            ctl.set_color(zone, parse_hex(spec["color"]))
        elif mode == "effect":
            ctl.set_effect(zone, spec["effect"], parse_hex(spec["color"]), spec["brightness"], spec["speed"])
        elif mode == "rainbow":
            # primeiro quadro na hora; o arco-íris estático (speed 0) é só esse quadro
            self.scene[name] = spec
            self._draw_rainbow(name, self.phase(spec["speed"]))
        else:
            ctl.off(zone)
        self.scene[name] = spec

    def _apply_many(self, names, spec):
        for name in names:
            self._apply(name, dict(spec))
        self._changed()

    def _changed(self):
        self.config.set_scene({n: dict(s) for n, s in self.scene.items()})
        self._notify()

    def _notify(self):
        for listener in self.listeners:
            listener()
