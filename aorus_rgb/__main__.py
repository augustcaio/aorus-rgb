import argparse
import sys
import time

from .device import EFFECTS, Controller, DeviceNotFound


def parse_color(s):
    s = s.lstrip("#")
    if len(s) != 6:
        raise argparse.ArgumentTypeError("use hex RRGGBB, ex: ff0000")
    return tuple(int(s[i:i + 2], 16) for i in (0, 2, 4))


def cmd_info(c, a):
    i = c.info
    print(f"dispositivo : {c.path}")
    print(f"produto     : {i['product']}")
    print(f"firmware    : {i['fw']}")
    print(f"strip detect: {i['strip_detect']:#04x}")
    print(f"LEDs ARGB   : D_LED1={i['argb_counts'][0]}  D_LED2={i['argb_counts'][1]}")
    print(f"ordem cor   : D_LED1={i['cal'][0]:#08x}  D_LED2={i['cal'][1]:#08x}")


def cmd_static(c, a):
    c.builtin_effects(a.header, False)
    c.set_led_count(a.header, a.leds)
    c.set_strip(a.header, [a.color] * a.leds)


def cmd_off(c, a):
    for h in (1, 2):
        c.builtin_effects(h, False)
        c.set_strip(h, [(0, 0, 0)] * 256)


def cmd_effect(c, a):
    c.set_effect(-1, a.name, a.color, a.brightness, a.speed)


def cmd_rainbow(c, a):
    import colorsys
    c.builtin_effects(a.header, False)
    c.set_led_count(a.header, a.leds)
    t = 0.0
    try:
        while True:
            frame = [tuple(int(x * 255) for x in colorsys.hsv_to_rgb((t + i / a.leds) % 1, 1, a.brightness / 255))
                     for i in range(a.leds)]
            c.set_strip(a.header, frame)
            t = (t + a.speed / 1000) % 1
            time.sleep(1 / 30)
    except KeyboardInterrupt:
        pass


def main():
    p = argparse.ArgumentParser(prog="aorus-rgb", description="Controle RGB da B550M Aorus Elite")
    sub = p.add_subparsers(dest="cmd", required=True)
    sub.add_parser("info").set_defaults(fn=cmd_info)
    sub.add_parser("off").set_defaults(fn=cmd_off)

    s = sub.add_parser("static", help="cor fixa nos LEDs endereçáveis")
    s.add_argument("color", type=parse_color)
    s.add_argument("--header", type=int, choices=(1, 2), default=1)
    s.add_argument("--leds", type=int, default=32)
    s.set_defaults(fn=cmd_static)

    s = sub.add_parser("rainbow", help="arco-íris animado (Ctrl+C para sair)")
    s.add_argument("--header", type=int, choices=(1, 2), default=1)
    s.add_argument("--leds", type=int, default=32)
    s.add_argument("--speed", type=float, default=5)
    s.add_argument("--brightness", type=int, default=255)
    s.set_defaults(fn=cmd_rainbow)

    s = sub.add_parser("effect", help="efeito de hardware nos LEDs RGB comuns/placa")
    s.add_argument("name", choices=EFFECTS)
    s.add_argument("color", type=parse_color)
    s.add_argument("--brightness", type=int, default=255)
    s.add_argument("--speed", type=int, default=5)
    s.set_defaults(fn=cmd_effect)

    a = p.parse_args()
    try:
        with Controller() as c:
            a.fn(c, a)
    except DeviceNotFound as e:
        sys.exit(f"erro: {e}")
    except PermissionError:
        sys.exit("erro: sem permissão no hidraw. Instale a regra udev (veja README).")


if __name__ == "__main__":
    main()
