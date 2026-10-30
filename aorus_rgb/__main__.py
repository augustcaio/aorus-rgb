import argparse
import os
import sys
import time
from contextlib import ExitStack

from . import protocol as p
from .client import connect
from .device import Controller, DeviceNotFound, find_hidraw
from .dram import DramController, DramZone
from .patterns import (
    CHECK_PALETTE,
    DRAM_FRAME_DIVIDER,
    RAINBOW_FPS,
    light_first,
    palette_colors,
    rainbow_frame,
)
from .smbus import BusNotFound, ForbiddenAddress, find_dram_bus

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


def cmd_rainbow(devs, a):
    """Onda de arco-íris gerada pelo PC; com `all`, headers ARGB e memórias juntos.

    Tudo é calculado com a mesma fase a cada quadro, então placa e memórias
    ficam sincronizadas. O rainbow do próprio chip ENE não é usado: tem
    relógio próprio e sai de sincronia com as fans.
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
    t = 0.0
    frame = 0
    deadline = time.monotonic()
    try:
        while True:
            for ctl, z in targets:
                if isinstance(z, DramZone) and frame % DRAM_FRAME_DIVIDER:
                    continue
                ctl.set_colors(z, rainbow_frame(ctl.led_count(z), t, v))
            frame += 1
            t = (t + a.speed / 1000) % 1
            deadline += 1 / RAINBOW_FPS
            time.sleep(max(0.0, deadline - time.monotonic()))
    except KeyboardInterrupt:
        pass


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
        print(check_legend(z.name, n))


def check_legend(name, n):
    colors = ", ".join(f"{i + 1} {CHECK_PALETTE[i % len(CHECK_PALETTE)][0]}" for i in range(n))
    return f"{name}: {n} LEDs -> {colors}"


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


def search_led_count(name, light):
    """Busca binária guiada pelo menor N que acende todos os LEDs.

    Funciona com fans em cadeia (N = soma dos LEDs) e em splitter/hub paralelo
    (N = LEDs da fan com mais LEDs), sem depender de marca ou modelo. Usa cores
    distintas: com tudo branco, o brilho vaza pelo difusor e esconde LEDs
    apagados (deu 10 para fans de 12 LEDs na B550M AORUS ELITE).
    """
    light(CALIBRATE_MAX)
    if not ask_yes(f"{name}: com {CALIBRATE_MAX} LEDs, todos os LEDs de todas as fans estão acesos?"):
        raise ValueError(f"{name}: nem todos acendem com {CALIBRATE_MAX} LEDs; confira os cabos")
    lo, hi = 1, CALIBRATE_MAX
    while lo < hi:
        mid = (lo + hi) // 2
        light(mid)
        if ask_yes(f"{name}: [{mid} LEDs] todos os LEDs de todas as fans estão acesos (nenhum apagado)?"):
            hi = mid
        else:
            lo = mid + 1
    return lo


def cmd_calibrate(devs, a):
    c, (z,) = board_argb(a.zone, devs)
    count = search_led_count(z.name, lambda n: light_first(c, z, n, palette_colors(n)))
    c.set_chip_led_count(z, count)
    c.config.set_leds(z.name, count)
    light_first(c, z, count, palette_colors(count))
    print(f"{z.name}: {count} LEDs, salvo em {c.config.path} (confira: uma cor por LED, nenhum apagado)")


def cmd_profile(devs, a):
    raise ValueError("perfis precisam do daemon: systemctl --user start aorus-rgb")


RUN_WAIT_SECONDS = 15


def wait_for_device_access(timeout=RUN_WAIT_SECONDS, poll=0.25):
    """Espera a placa e o SMBus das memórias ficarem acessíveis; devolve o que faltou.

    No login, o serviço pode iniciar antes de o logind aplicar a ACL da regra
    udev (uaccess): a primeira tentativa falhava com "sem permissão". Esperar
    pelos dois evita também começar o `rainbow all` sem as memórias.
    """
    deadline = time.monotonic() + timeout
    while True:
        pending = []
        try:
            board = find_hidraw()
        except DeviceNotFound:
            pending.append("placa (ainda não detectada)")
        else:
            if not os.access(board, os.R_OK | os.W_OK):
                pending.append(board)
        try:
            bus = find_dram_bus()
        except BusNotFound:
            pass  # sem SMBus (ex.: sem i2c-dev): segue sem as memórias
        else:
            if not os.access(bus, os.R_OK | os.W_OK):
                pending.append(bus)
        if not pending or time.monotonic() >= deadline:
            return pending
        time.sleep(poll)


def cmd_daemon(devs, a):
    """Dono do hardware: restaura a última cena e atende a CLI e a interface pelo D-Bus."""
    try:
        from . import dbus_service
    except ImportError:
        raise ValueError("o daemon precisa do PyGObject (pacote python-gobject)") from None
    from .engine import Engine

    pending = wait_for_device_access()
    if pending:
        warn(f"sem acesso após {RUN_WAIT_SECONDS}s: {', '.join(pending)}")
    board = devs.get("board")
    engine = Engine(board, devs.get("dram", required=False), board.config)
    engine.restore()
    code = dbus_service.run(engine)
    if code:
        sys.exit(code)


# --- comandos pelo daemon ------------------------------------------------------
LEDS_NEED_DIRECT = "--leds muda a capacidade do chip e pede o daemon parado; use calibrate ou check --leds"


def to_hex(color):
    return "".join(f"{c:02x}" for c in color)


def describe(spec):
    if spec is None:
        return "-"
    mode = spec["mode"]
    if mode == "color":
        return f"cor #{spec['color']}"
    if mode == "effect":
        return f"{spec['effect']} #{spec['color']}, velocidade {spec['speed']}, brilho {spec['brightness']}"
    if mode == "rainbow":
        return "arco-íris estático" if spec["speed"] == 0 else f"arco-íris, velocidade {spec['speed']:g}"
    return "desligado"


def daemon_info(client, a):
    info = client.call("GetDeviceInfo")[0]
    state = client.call("GetState")[0]
    print("daemon      : ativo")
    print(f"produto     : {info.get('board.product', '-')}")
    print(f"firmware    : {info.get('board.firmware', '-')}")
    print("zonas       :")
    for name, kind, leds in client.call("GetZones")[0]:
        leds_label = f"{leds:>3} LED{'s' if leds != 1 else ' '}"
        print(f"  {name:<8} {kind.upper():<5} {leds_label}  {describe(state.get(name))}")


def daemon_color(client, a):
    if a.leds:
        raise ValueError(LEDS_NEED_DIRECT)
    client.call("SetColor", "(ss)", a.zone, to_hex(a.color))


def daemon_effect(client, a):
    client.call("SetEffect", "(sssuu)", a.zone, a.name, to_hex(a.color), a.brightness, a.speed)


def daemon_rainbow(client, a):
    if a.leds:
        raise ValueError(LEDS_NEED_DIRECT)
    client.call("SetRainbow", "(sdu)", a.zone, float(a.speed), a.brightness)


def daemon_off(client, a):
    client.call("Off", "(s)", a.zone)


def daemon_count(client, a):
    raise ValueError(f"com o daemon rodando, use: aorus-rgb check {a.zone} --leds {a.n}")


def daemon_check(client, a):
    zones = client.zones()
    names = ARGB_CHOICES if a.zone == "all" else [a.zone]
    for name in names:
        n = a.leds or zones[name][1]
        client.call("LightFirst", "(su)", name, n)
        print(check_legend(name, n))
    try:
        input("Enter para voltar ao estado anterior... ")
    except EOFError:
        return  # sem terminal: o padrão fica até o próximo comando nessa zona
    for name in names:
        client.call("EndCalibration", "(s)", name)


def daemon_calibrate(client, a):
    count = search_led_count(a.zone, lambda n: client.call("LightFirst", "(su)", a.zone, n))
    client.call("SetLedCount", "(su)", a.zone, count)
    print(f"{a.zone}: {count} LEDs, salvo pelo daemon")


def daemon_profile(client, a):
    if a.action == "list":
        names = client.call("ListProfiles")[0]
        print("\n".join(names) if names else "(nenhum perfil salvo)")
        return
    if not a.name:
        raise ValueError(f"informe o nome do perfil: aorus-rgb profile {a.action} NOME")
    method = {"save": "SaveProfile", "apply": "ApplyProfile", "delete": "DeleteProfile"}[a.action]
    client.call(method, "(s)", a.name)


DAEMON_COMMANDS = {
    "info": daemon_info,
    "color": daemon_color,
    "effect": daemon_effect,
    "rainbow": daemon_rainbow,
    "off": daemon_off,
    "count": daemon_count,
    "check": daemon_check,
    "calibrate": daemon_calibrate,
    "profile": daemon_profile,
}


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

    s = sub.add_parser("profile", help="perfis com nome, guardados pelo daemon")
    s.add_argument("action", choices=["list", "save", "apply", "delete"], nargs="?", default="list")
    s.add_argument("name", nargs="?", help="nome do perfil (save, apply e delete)")
    s.set_defaults(fn=cmd_profile)

    sub.add_parser("daemon", help="daemon D-Bus: dono do hardware, usado pelo serviço e pela interface").set_defaults(
        fn=cmd_daemon
    )

    s = sub.add_parser("off", help="apaga uma zona")
    s.add_argument("zone", choices=ZONE_CHOICES, nargs="?", default="all")
    s.set_defaults(fn=cmd_off)
    return ap


def main(argv=None):
    a = build_parser().parse_args(argv)
    try:
        client = connect() if a.cmd in DAEMON_COMMANDS else None
        if client is not None:
            DAEMON_COMMANDS[a.cmd](client, a)
            return
        with ExitStack() as stack:
            a.fn(Devices(stack), a)
    except (DeviceNotFound, BusNotFound, ForbiddenAddress, ValueError) as e:
        sys.exit(f"erro: {e}")
    except PermissionError as e:
        sys.exit(f"erro: sem permissão em {e.filename}. Instale a regra udev (veja docs/desenvolvimento.md na main).")


if __name__ == "__main__":
    main()
