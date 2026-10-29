import argparse
import colorsys
import sys
import time
from contextlib import ExitStack

from . import protocol as p
from .device import Controller, DeviceNotFound
from .dram import DramController
from .smbus import BusNotFound, ForbiddenAddress

DRAM_SLOTS = [f"dram{i}" for i in range(1, 5)]
ZONE_CHOICES = [*p.ZONES, *DRAM_SLOTS, "dram", "all"]
ANIMATABLE = [name for name, z in p.ZONES.items() if z.addressable] + DRAM_SLOTS


def parse_color(s):
    s = s.lstrip("#")
    try:
        if len(s) != 6:
            raise ValueError
        return tuple(int(s[i:i + 2], 16) for i in (0, 2, 4))
    except ValueError:
        raise argparse.ArgumentTypeError("use hex RRGGBB, ex: ff0000") from None


def warn(msg):
    print(f"aviso: {msg}", file=sys.stderr)


class Devices:
    """Abre a placa (IT5702) e as memórias (SMBus) só quando um comando precisa."""

    def __init__(self, stack, board_factory=Controller, dram_factory=DramController):
        self.stack = stack
        self._factories = {"board": board_factory, "dram": dram_factory}
        self._open = {}

    def get(self, kind, required=True):
        if kind not in self._open:
            try:
                self._open[kind] = self.stack.enter_context(self._factories[kind]())
            except (DeviceNotFound, BusNotFound, OSError) as e:
                if required:
                    raise
                warn(f"{'memórias' if kind == 'dram' else 'placa'} indisponível: {e}")
                self._open[kind] = None
        return self._open[kind]

    def opened(self):
        return [c for c in self._open.values() if c is not None]


def resolve(name, devs):
    """Nome de zona da CLI -> lista de (controlador, zona)."""
    if name in p.ZONES:
        return [(devs.get("board"), p.ZONES[name])]
    if name.startswith("dram"):
        dram = devs.get("dram")
        if name == "dram":
            return [(dram, z) for z in dram.zones.values()]
        if name not in dram.zones:
            found = ", ".join(dram.zones) or "nenhuma"
            raise ValueError(f"{name} não encontrado (memórias detectadas: {found})")
        return [(dram, dram.zones[name])]
    # all: a placa é obrigatória, as memórias são opcionais
    pairs = [(devs.get("board"), z) for z in p.ZONES.values()]
    dram = devs.get("dram", required=False)
    if dram:
        pairs += [(dram, z) for z in dram.zones.values()]
    return pairs


def cmd_info(devs, a):
    board = devs.get("board", required=False)
    if board:
        i = board.info
        print(f"placa       : {board.path}")
        print(f"produto     : {i.product}")
        print(f"firmware    : {i.firmware}")
        print(f"chip id     : {i.chip_id:#010x}")
        print(f"strip detect: {i.strip_detect:#04x}")
        print("zonas       :")
        for z in p.ZONES.values():
            if z.addressable:
                order = p.color_order_name(i.calibration[z.cal_slot])
                print(f"  {z.name:<8} ARGB  {board.led_count(z)} LEDs, ordem {order}")
            else:
                print(f"  {z.name:<8} RGB   cor única")
    dram = devs.get("dram", required=False)
    if dram:
        print(f"memórias    : {dram.path}")
        if not dram.zones:
            print("  nenhum controlador ENE suportado encontrado")
        for z in dram.zones.values():
            print(f"  {z.name:<8} ENE   {z.device.led_count} LEDs, {z.device.version} em {z.device.address:#04x}")


def cmd_color(devs, a):
    for ctl, z in resolve(a.zone, devs):
        if a.leds and isinstance(z, p.Zone) and z.addressable:
            ctl.set_led_count(z, a.leds)
        ctl.set_color(z, a.color)


def cmd_effect(devs, a):
    for ctl, z in resolve(a.zone, devs):
        ctl.set_effect(z, a.name, a.color, a.brightness, a.speed, apply=False)
    for ctl in devs.opened():
        ctl.apply()


def cmd_off(devs, a):
    for ctl, z in resolve(a.zone, devs):
        ctl.off(z)


def cmd_rainbow(devs, a):
    ((ctl, z),) = resolve(a.zone, devs)
    if a.leds and isinstance(z, p.Zone):
        ctl.set_led_count(z, a.leds)
    n = ctl.led_count(z)
    v = a.brightness / 255
    t = 0.0
    try:
        while True:
            frame = [tuple(int(x * 255) for x in colorsys.hsv_to_rgb((t + i / n) % 1, 1, v)) for i in range(n)]
            ctl.set_colors(z, frame)
            t = (t + a.speed / 1000) % 1
            time.sleep(1 / 30)
    except KeyboardInterrupt:
        pass


def build_parser():
    ap = argparse.ArgumentParser(prog="aorus-rgb", description="Controle RGB da B550M Aorus Elite e das memórias")
    sub = ap.add_subparsers(dest="cmd", required=True)

    sub.add_parser("info", help="informações dos controladores e das zonas").set_defaults(fn=cmd_info)

    s = sub.add_parser("color", help="cor fixa numa zona")
    s.add_argument("zone", choices=ZONE_CHOICES)
    s.add_argument("color", type=parse_color)
    s.add_argument("--leds", type=int, help="quantidade de LEDs do strip ARGB")
    s.set_defaults(fn=cmd_color)

    s = sub.add_parser("effect", help="efeito de hardware (roda no chip)")
    s.add_argument("zone", choices=ZONE_CHOICES)
    s.add_argument("name", choices=[e for e in p.EFFECTS if e != "none"])
    s.add_argument("color", type=parse_color)
    s.add_argument("--brightness", type=int, default=255, help="0-255")
    s.add_argument("--speed", type=int, default=5, help=f"0-{p.SPEED_MAX}")
    s.set_defaults(fn=cmd_effect)

    s = sub.add_parser("rainbow", help="arco-íris animado (Ctrl+C para sair)")
    s.add_argument("zone", choices=ANIMATABLE)
    s.add_argument("--leds", type=int)
    s.add_argument("--speed", type=float, default=5)
    s.add_argument("--brightness", type=int, default=255)
    s.set_defaults(fn=cmd_rainbow)

    s = sub.add_parser("off", help="apaga uma zona")
    s.add_argument("zone", choices=ZONE_CHOICES, nargs="?", default="all")
    s.set_defaults(fn=cmd_off)
    return ap


def main(argv=None):
    a = build_parser().parse_args(argv)
    try:
        with ExitStack() as stack:
            a.fn(Devices(stack), a)
    except (DeviceNotFound, BusNotFound, ForbiddenAddress, ValueError) as e:
        sys.exit(f"erro: {e}")
    except PermissionError as e:
        sys.exit(f"erro: sem permissão em {e.filename}. Instale a regra udev (veja docs/desenvolvimento.md na main).")


if __name__ == "__main__":
    main()
