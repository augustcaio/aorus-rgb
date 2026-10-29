import argparse
import colorsys
import sys
import time

from . import protocol as p
from .device import Controller, DeviceNotFound

ZONE_CHOICES = [*p.ZONES, "all"]
ARGB_CHOICES = [name for name, z in p.ZONES.items() if z.addressable]


def parse_color(s):
    s = s.lstrip("#")
    try:
        if len(s) != 6:
            raise ValueError
        return tuple(int(s[i:i + 2], 16) for i in (0, 2, 4))
    except ValueError:
        raise argparse.ArgumentTypeError("use hex RRGGBB, ex: ff0000") from None


def zones(name):
    return list(p.ZONES.values()) if name == "all" else [p.ZONES[name]]


def cmd_info(c, a):
    i = c.info
    print(f"dispositivo : {c.path}")
    print(f"produto     : {i.product}")
    print(f"firmware    : {i.firmware}")
    print(f"chip id     : {i.chip_id:#010x}")
    print(f"strip detect: {i.strip_detect:#04x}")
    print("zonas       :")
    for z in p.ZONES.values():
        if z.addressable:
            order = p.color_order_name(i.calibration[z.cal_slot])
            print(f"  {z.name:<8} ARGB  {c.led_count(z)} LEDs, ordem {order}")
        else:
            print(f"  {z.name:<8} RGB   cor única")


def cmd_color(c, a):
    for z in zones(a.zone):
        if a.leds and z.addressable:
            c.set_led_count(z, a.leds)
        c.set_color(z, a.color)


def cmd_effect(c, a):
    for z in zones(a.zone):
        c.set_effect(z, a.name, a.color, a.brightness, a.speed, apply=False)
    c.apply()


def cmd_off(c, a):
    for z in zones(a.zone):
        c.off(z)


def cmd_rainbow(c, a):
    z = p.ZONES[a.zone]
    if a.leds:
        c.set_led_count(z, a.leds)
    n = c.led_count(z)
    v = a.brightness / 255
    t = 0.0
    try:
        while True:
            frame = [tuple(int(x * 255) for x in colorsys.hsv_to_rgb((t + i / n) % 1, 1, v)) for i in range(n)]
            c.set_colors(z, frame)
            t = (t + a.speed / 1000) % 1
            time.sleep(1 / 30)
    except KeyboardInterrupt:
        pass


def build_parser():
    ap = argparse.ArgumentParser(prog="aorus-rgb", description="Controle RGB da B550M Aorus Elite (ITE IT5702)")
    sub = ap.add_subparsers(dest="cmd", required=True)

    sub.add_parser("info", help="informações do controlador e das zonas").set_defaults(fn=cmd_info)

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

    s = sub.add_parser("rainbow", help="arco-íris animado num strip ARGB (Ctrl+C para sair)")
    s.add_argument("zone", choices=ARGB_CHOICES)
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
        with Controller() as c:
            a.fn(c, a)
    except DeviceNotFound as e:
        sys.exit(f"erro: {e}")
    except PermissionError:
        sys.exit("erro: sem permissão no hidraw. Instale a regra udev (veja docs/desenvolvimento.md na main).")
    except ValueError as e:
        sys.exit(f"erro: {e}")


if __name__ == "__main__":
    main()
