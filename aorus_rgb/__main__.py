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
            calibrated = c.config.leds(z.name)
            leds = f"{calibrated} LEDs (calibrado)" if calibrated else f"{c.led_count(z)} LEDs (padrão; rode calibrate)"
            print(f"  {z.name:<8} ARGB  {leds}, ordem {order}")
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


def rainbow_frame(n, t, value):
    """Arco-íris de n LEDs com fase t (0-1)."""
    return [tuple(int(x * 255) for x in colorsys.hsv_to_rgb((t + i / n) % 1, 1, value)) for i in range(n)]


def cmd_rainbow(c, a):
    """Onda de arco-íris gerada pelo PC; com `all`, todos os headers ARGB juntos."""
    targets = [z for z in zones(a.zone) if z.addressable]
    if a.leds:
        for z in targets:
            c.set_led_count(z, a.leds)
    v = a.brightness / 255
    if a.speed == 0:
        # gradiente fixo: as cores diretas ficam no chip depois que o comando sai
        for z in targets:
            c.set_colors(z, rainbow_frame(c.led_count(z), 0.0, v))
        return
    t = 0.0
    try:
        while True:
            for z in targets:
                c.set_colors(z, rainbow_frame(c.led_count(z), t, v))
            t = (t + a.speed / 1000) % 1
            time.sleep(1 / 30)
    except KeyboardInterrupt:
        pass


# cores bem distintas entre vizinhos, para conferir LED por LED
CHECK_PALETTE = (
    ("vermelho", (255, 0, 0)),
    ("laranja", (255, 80, 0)),
    ("amarelo", (255, 200, 0)),
    ("verde", (0, 255, 0)),
    ("ciano", (0, 255, 255)),
    ("azul", (0, 0, 255)),
    ("roxo", (140, 0, 255)),
    ("rosa", (255, 0, 120)),
    ("branco", (255, 255, 255)),
    ("verde-limão", (120, 255, 0)),
    ("azul-claro", (0, 120, 255)),
    ("magenta", (255, 0, 255)),
)


def palette_colors(n):
    return [CHECK_PALETTE[i % len(CHECK_PALETTE)][1] for i in range(n)]


def light_first(c, z, n, colors=None):
    """Acende os primeiros n LEDs (branco, ou as cores dadas) e apaga o resto do chip."""
    if n > c.chip_led_count(z):
        c.set_chip_led_count(z, n)
    lit = colors or [(255, 255, 255)] * n
    c.set_colors(z, lit + [(0, 0, 0)] * (c.chip_led_count(z) - n))


def cmd_count(c, a):
    light_first(c, p.ZONES[a.zone], a.n)


def cmd_check(c, a):
    """Uma cor diferente por LED até o número calibrado; o resto apagado."""
    for z in [z for z in zones(a.zone) if z.addressable]:
        if a.leds:
            c.set_led_count(z, a.leds)
        n = c.led_count(z)
        light_first(c, z, n, palette_colors(n))
        legend = ", ".join(f"{i + 1} {CHECK_PALETTE[i % len(CHECK_PALETTE)][0]}" for i in range(n))
        print(f"{z.name}: {n} LEDs -> {legend}")


def ask_yes(question):
    while True:
        try:
            answer = input(f"{question} [s/n] ").strip().lower()
        except EOFError:
            raise ValueError("calibrate é interativo: rode num terminal") from None
        if answer in ("s", "sim", "y", "yes"):
            return True
        if answer in ("n", "nao", "não", "no"):
            return False


CALIBRATE_MAX = p.LED_COUNT_STEPS[2]  # 256: limite dos headers do IT57xx


def cmd_calibrate(c, a):
    """Busca binária guiada pelo menor N que acende todos os LEDs; salva o resultado.

    Funciona com fans em cadeia (N = soma dos LEDs) e em splitter/hub paralelo
    (N = LEDs da fan com mais LEDs), sem depender de marca ou modelo.
    """
    z = p.ZONES[a.zone]
    # cores distintas: com tudo branco, o brilho vaza pelo difusor e esconde
    # LEDs apagados (deu 10 para fans de 12 LEDs na B550M AORUS ELITE)
    light_first(c, z, CALIBRATE_MAX, palette_colors(CALIBRATE_MAX))
    if not ask_yes(f"{z.name}: com {CALIBRATE_MAX} LEDs, todos os LEDs de todas as fans estão acesos?"):
        raise ValueError(f"{z.name}: nem todos acendem com {CALIBRATE_MAX} LEDs; confira os cabos")
    lo, hi = 1, CALIBRATE_MAX
    while lo < hi:
        mid = (lo + hi) // 2
        light_first(c, z, mid, palette_colors(mid))
        if ask_yes(f"{z.name}: [{mid} LEDs] todos os LEDs de todas as fans estão acesos (nenhum apagado)?"):
            hi = mid
        else:
            lo = mid + 1
    c.set_chip_led_count(z, lo)
    c.config.set_leds(z.name, lo)
    light_first(c, z, lo, palette_colors(lo))
    print(f"{z.name}: {lo} LEDs, salvo em {c.config.path} (confira: uma cor por LED, nenhum apagado)")


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

    s = sub.add_parser("rainbow", help="onda de arco-íris gerada pelo PC (Ctrl+C para sair)")
    s.add_argument("zone", choices=[*ARGB_CHOICES, "all"])
    s.add_argument("--leds", type=int)
    s.add_argument("--speed", type=float, default=5, help="0 = arco-íris estático (pinta uma vez e sai)")
    s.add_argument("--brightness", type=int, default=255)
    s.set_defaults(fn=cmd_rainbow)

    s = sub.add_parser("count", help="acende os primeiros N LEDs em branco, para contar a faixa")
    s.add_argument("zone", choices=ARGB_CHOICES)
    s.add_argument("n", type=int, choices=range(1, p.LED_COUNT_STEPS[2] + 1), metavar="N",
                   help=f"1-{p.LED_COUNT_STEPS[2]}")
    s.set_defaults(fn=cmd_count)

    s = sub.add_parser("check", help="uma cor diferente por LED, para conferir a calibração")
    s.add_argument("zone", choices=[*ARGB_CHOICES, "all"])
    s.add_argument("--leds", type=int, help="testa outra quantidade sem mudar a calibração")
    s.set_defaults(fn=cmd_check)

    s = sub.add_parser("calibrate", help="descobre e salva quantos LEDs um header ARGB tem (interativo)")
    s.add_argument("zone", choices=ARGB_CHOICES)
    s.set_defaults(fn=cmd_calibrate)

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
