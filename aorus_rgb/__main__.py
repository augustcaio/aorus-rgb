import argparse
import colorsys
import sys
import time
from contextlib import ExitStack

from . import protocol as p
from .config import Config
from .device import Controller, DeviceNotFound
from .dram import DramController, DramZone
from .smbus import BusNotFound, ForbiddenAddress

DRAM_SLOTS = [f"dram{i}" for i in range(1, 5)]
ZONE_CHOICES = [*p.ZONES, *DRAM_SLOTS, "dram", "all"]
ARGB_CHOICES = [name for name, z in p.ZONES.items() if z.addressable]
ANIMATABLE = [*ARGB_CHOICES, *DRAM_SLOTS, "dram", "all"]


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


def board_argb(name, devs):
    """Headers ARGB da placa: um pelo nome, ou todos com `all`."""
    board = devs.get("board")
    names = ARGB_CHOICES if name == "all" else [name]
    return board, [p.ZONES[n] for n in names]


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
                calibrated = board.config.leds(z.name)
                leds = (
                    f"{calibrated} LEDs (calibrado)" if calibrated
                    else f"{board.led_count(z)} LEDs (padrão; rode calibrate)"
                )
                print(f"  {z.name:<8} ARGB  {leds}, ordem {order}")
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


def rainbow_frame(n, t, value):
    """Arco-íris de n LEDs com fase t (0-1)."""
    return [tuple(int(x * 255) for x in colorsys.hsv_to_rgb((t + i / n) % 1, 1, value)) for i in range(n)]


RAINBOW_FPS = 30


def cmd_rainbow(devs, a):
    """Onda de arco-íris; com `all`, headers ARGB e memórias juntos.

    Estático (--speed 0): o PC pinta tudo uma vez. Animado: os headers ARGB
    são animados pelo PC (USB, ~9 ms por quadro) e as memórias usam o rainbow
    do próprio chip ENE. Animá-las pelo PC ocuparia o SMBus em ~36% do tempo,
    disputando o barramento com o BIOS (acpi_enforce_resources=lax).
    """
    targets = [(ctl, z) for ctl, z in resolve(a.zone, devs) if z.addressable]
    if a.leds:
        for ctl, z in targets:
            if isinstance(z, p.Zone):
                ctl.set_led_count(z, a.leds)
    v = a.brightness / 255
    if a.speed == 0:
        # gradiente fixo: as cores diretas ficam nos chips depois que o comando sai
        for ctl, z in targets:
            ctl.set_colors(z, rainbow_frame(ctl.led_count(z), 0.0, v))
        return
    for ctl, z in targets:
        if isinstance(z, DramZone):
            ctl.set_rainbow(z, a.speed)
    animated = [(ctl, z) for ctl, z in targets if not isinstance(z, DramZone)]
    if not animated:
        return
    t = 0.0
    deadline = time.monotonic()
    try:
        while True:
            for ctl, z in animated:
                ctl.set_colors(z, rainbow_frame(ctl.led_count(z), t, v))
            t = (t + a.speed / 1000) % 1
            deadline += 1 / RAINBOW_FPS
            time.sleep(max(0.0, deadline - time.monotonic()))
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


def cmd_count(devs, a):
    board, (z,) = board_argb(a.zone, devs)
    light_first(board, z, a.n)


def cmd_check(devs, a):
    """Uma cor diferente por LED até o número calibrado; o resto apagado."""
    board, targets = board_argb(a.zone, devs)
    for z in targets:
        if a.leds:
            board.set_led_count(z, a.leds)
        n = board.led_count(z)
        light_first(board, z, n, palette_colors(n))
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


def cmd_calibrate(devs, a):
    """Busca binária guiada pelo menor N que acende todos os LEDs; salva o resultado.

    Funciona com fans em cadeia (N = soma dos LEDs) e em splitter/hub paralelo
    (N = LEDs da fan com mais LEDs), sem depender de marca ou modelo.
    """
    c, (z,) = board_argb(a.zone, devs)
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


PROFILE_COMMANDS = ("color", "effect", "rainbow", "off")
DEFAULT_PROFILE = ["rainbow", "all"]


def cmd_profile(devs, a):
    """Mostra ou salva o comando que `run` (e o serviço) aplica."""
    config = Config()
    if not a.command:
        profile = config.profile()
        print(" ".join(profile) if profile else f"(nenhum; padrão: {' '.join(DEFAULT_PROFILE)})")
        return
    if a.command[0] not in PROFILE_COMMANDS:
        raise ValueError(f"o perfil deve ser um destes comandos: {', '.join(PROFILE_COMMANDS)}")
    try:
        build_parser().parse_args(a.command)
    except SystemExit:
        raise ValueError(f"perfil inválido: {' '.join(a.command)}") from None
    config.set_profile(a.command)
    print(f"perfil salvo: {' '.join(a.command)}")


def cmd_run(devs, a):
    """Aplica o perfil salvo (é o que o serviço do systemd executa)."""
    argv = Config().profile() or DEFAULT_PROFILE
    profile = build_parser().parse_args(argv)
    profile.fn(devs, profile)


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

    s = sub.add_parser("rainbow", help="onda de arco-íris gerada pelo PC (Ctrl+C para sair)")
    s.add_argument("zone", choices=ANIMATABLE)
    s.add_argument("--leds", type=int)
    s.add_argument("--speed", type=float, default=5, help="0 = arco-íris estático (pinta uma vez e sai)")
    s.add_argument("--brightness", type=int, default=255)
    s.set_defaults(fn=cmd_rainbow)

    s = sub.add_parser("count", help="acende os primeiros N LEDs em branco, para contar a faixa")
    s.add_argument("zone", choices=ARGB_CHOICES)
    s.add_argument("n", type=int, choices=range(1, CALIBRATE_MAX + 1), metavar="N", help=f"1-{CALIBRATE_MAX}")
    s.set_defaults(fn=cmd_count)

    s = sub.add_parser("check", help="uma cor diferente por LED, para conferir a calibração")
    s.add_argument("zone", choices=[*ARGB_CHOICES, "all"])
    s.add_argument("--leds", type=int, help="testa outra quantidade sem mudar a calibração")
    s.set_defaults(fn=cmd_check)

    s = sub.add_parser("calibrate", help="descobre e salva quantos LEDs um header ARGB tem (interativo)")
    s.add_argument("zone", choices=ARGB_CHOICES)
    s.set_defaults(fn=cmd_calibrate)

    s = sub.add_parser("profile", help="mostra ou salva o comando aplicado por `run` e pelo serviço")
    s.add_argument("command", nargs=argparse.REMAINDER, help="ex.: rainbow all --speed 3")
    s.set_defaults(fn=cmd_profile)

    sub.add_parser("run", help="aplica o perfil salvo (usado pelo serviço)").set_defaults(fn=cmd_run)

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
