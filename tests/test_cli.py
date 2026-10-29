import argparse
import time
from contextlib import ExitStack

import pytest
from fake_smbus import FakeEne, FakeSMBus

from aorus_rgb import ene
from aorus_rgb import protocol as p
from aorus_rgb.__main__ import (
    CHECK_PALETTE,
    Devices,
    build_parser,
    cmd_calibrate,
    cmd_check,
    cmd_count,
    cmd_profile,
    cmd_rainbow,
    cmd_run,
    parse_color,
    rainbow_frame,
    resolve,
    wait_for_device_access,
)
from aorus_rgb.config import Config
from aorus_rgb.device import DeviceNotFound
from aorus_rgb.dram import DramController
from aorus_rgb.smbus import BusNotFound


def devices(board, dram=None):
    def no_access():
        raise PermissionError(13, "Permission denied", "/dev/i2c-0")

    return Devices(ExitStack(), board_factory=lambda: board, dram_factory=(lambda: dram) if dram else no_access)


def run(cmd, board, *argv, dram=None):
    cmd(devices(board, dram), build_parser().parse_args(list(argv)))


# --- argumentos -----------------------------------------------------------

def test_parse_color():
    assert parse_color("ff8000") == (255, 128, 0)
    assert parse_color("#00ff00") == (0, 255, 0)


@pytest.mark.parametrize("bad", ["fff", "gg0000", "ff00001"])
def test_parse_color_rejects_invalid(bad):
    with pytest.raises(argparse.ArgumentTypeError):
        parse_color(bad)


def test_rainbow_rejects_single_color_zones():
    with pytest.raises(SystemExit):
        build_parser().parse_args(["rainbow", "led_c1"])


@pytest.mark.parametrize("zone", ["all", "dram", "dram1", "d_led2"])
def test_rainbow_accepts_argb_dram_and_all(zone):
    assert build_parser().parse_args(["rainbow", zone]).zone == zone


def test_off_defaults_to_all():
    assert build_parser().parse_args(["off"]).zone == "all"


def test_effect_no_longer_offers_hardware_wave():
    with pytest.raises(SystemExit):
        build_parser().parse_args(["effect", "all", "wave", "ff0000"])


def test_count_rejects_out_of_range():
    with pytest.raises(SystemExit):
        build_parser().parse_args(["count", "d_led1", "0"])


# --- zonas ----------------------------------------------------------------

def test_resolve_dram_zones(controller):
    devs = devices(controller, DramController(FakeSMBus([FakeEne(1), FakeEne(3)])))
    assert [z.name for _, z in resolve("dram", devs)] == ["dram1", "dram2"]
    assert [z.name for _, z in resolve("all", devs)][-2:] == ["dram1", "dram2"]
    with pytest.raises(ValueError, match="dram3"):
        resolve("dram3", devs)


def test_all_without_dram_access_warns_and_keeps_board(controller, capsys):
    pairs = resolve("all", devices(controller))
    assert [z.name for _, z in pairs] == ["d_led1", "d_led2", "led_c1", "led_cpu", "led_c2"]
    assert "memórias indisponível" in capsys.readouterr().err


def test_explicit_dram_without_access_is_an_error(controller):
    with pytest.raises(PermissionError):
        resolve("dram1", devices(controller))


# --- rainbow --------------------------------------------------------------

def test_rainbow_frame_spans_the_hue_circle():
    assert rainbow_frame(3, 0.0, 1.0) == [(255, 0, 0), (0, 255, 0), (0, 0, 255)]


def test_static_rainbow_paints_all_argb_headers_once(controller, transport):
    run(cmd_rainbow, controller, "rainbow", "all", "--speed", "0")  # retorna sem entrar no loop
    argb = [pkt for pkt in transport.sent if pkt[1] in (0x58, 0x59)]
    assert [pkt[1] for pkt in argb] == [0x58, 0x58, 0x59, 0x59]  # 32 LEDs = 2 pacotes por header
    assert argb[0][5:8] == bytes([0, 255, 0])  # primeiro LED vermelho (ordem GRB no fio)


def test_static_rainbow_all_includes_the_memory(controller):
    bus = FakeSMBus([FakeEne(1, led_count=8), FakeEne(3, led_count=8)])
    run(cmd_rainbow, controller, "rainbow", "all", "--speed", "0", dram=DramController(bus))
    for chip in bus.chips:
        r, b, g = chip.colors(0x8100, 1)
        assert (r, g, b) == (255, 0, 0)  # LED 1 vermelho nos dois pentes


# --- contagem, conferência e calibração ----------------------------------

def lit_leds(transport, reg=0x58):
    """Quantos LEDs do início da faixa o último quadro deixou acesos."""
    data = bytearray(256 * 3)
    for pkt in transport.sent:
        if pkt[1] == reg:
            offset = pkt[2] | pkt[3] << 8
            data[offset:offset + pkt[4]] = pkt[5:5 + pkt[4]]
    return next((i for i in range(256) if data[i * 3:i * 3 + 3] == bytes(3)), 256)


def answer_like_a_strip_of(physical, transport, monkeypatch):
    """Responde como o usuário olhando uma faixa com `physical` LEDs."""
    questions = []

    def fake_input(prompt):
        questions.append(prompt)
        return "s" if lit_leds(transport) >= physical else "n"

    monkeypatch.setattr("builtins.input", fake_input)
    return questions


def test_count_lights_first_n_white_and_clears_the_rest(controller, transport):
    run(cmd_count, controller, "count", "d_led1", "20")
    data = b"".join(pkt[5:5 + pkt[4]] for pkt in transport.sent if pkt[1] == 0x58)
    assert len(data) == 32 * 3
    assert data[:20 * 3] == b"\xff" * 60
    assert data[20 * 3:] == bytes(12 * 3)


def test_count_grows_the_strip_when_needed(controller):
    run(cmd_count, controller, "count", "d_led1", "40")
    assert controller.led_count(p.ZONES["d_led1"]) == 64


def test_check_paints_distinct_colors_and_clears_the_rest(controller, transport, capsys):
    controller.config.set_leds("d_led1", 10)
    run(cmd_check, controller, "check", "d_led1")
    assert lit_leds(transport) == 10
    assert "10 LEDs -> 1 vermelho, 2 laranja" in capsys.readouterr().out


def test_check_palette_neighbours_differ():
    colors = [rgb for _, rgb in CHECK_PALETTE]
    assert len(set(colors)) == len(colors)


@pytest.mark.parametrize("physical", [1, 9, 54, 256])
def test_calibrate_finds_and_saves_the_led_count(controller, transport, monkeypatch, physical):
    questions = answer_like_a_strip_of(physical, transport, monkeypatch)
    run(cmd_calibrate, controller, "calibrate", "d_led1")
    assert controller.config.leds("d_led1") == physical
    assert controller.led_count(p.ZONES["d_led1"]) == physical
    assert len(questions) <= 9  # 1 conferência + busca binária em 256


def test_calibrate_uses_distinct_colors_not_white(controller, transport, monkeypatch):
    answer_like_a_strip_of(12, transport, monkeypatch)
    run(cmd_calibrate, controller, "calibrate", "d_led1")
    first = next(pkt for pkt in transport.sent if pkt[1] == 0x58)
    assert first[5:8] != b"\xff\xff\xff"  # LED 1 não é branco
    assert controller.config.leds("d_led1") == 12


def test_calibrate_stops_when_256_is_not_enough(controller, monkeypatch):
    monkeypatch.setattr("builtins.input", lambda prompt: "n")
    with pytest.raises(ValueError, match="256"):
        run(cmd_calibrate, controller, "calibrate", "d_led1")
    assert controller.config.leds("d_led1") is None


def test_calibrate_needs_an_interactive_terminal(controller, monkeypatch):
    def no_stdin(prompt):
        raise EOFError

    monkeypatch.setattr("builtins.input", no_stdin)
    with pytest.raises(ValueError, match="terminal"):
        run(cmd_calibrate, controller, "calibrate", "d_led1")


# --- rainbow animado ------------------------------------------------------

def stop_after_frames(monkeypatch, frames=1):
    """Simula Ctrl+C depois de `frames` quadros, só no laço da CLI."""
    calls = []

    class Clock:
        monotonic = staticmethod(time.monotonic)

        @staticmethod
        def sleep(seconds):
            calls.append(seconds)
            if len(calls) >= frames:
                raise KeyboardInterrupt

    monkeypatch.setattr("aorus_rgb.__main__.time", Clock)


def stop_after_first_frame(monkeypatch):
    stop_after_frames(monkeypatch, 1)


def memory_frames(bus, address):
    return [e[3] for e in bus.log if e[:3] == ("block", address, 0x03)]


def test_animated_rainbow_streams_memory_in_phase_with_the_board(controller, transport, monkeypatch):
    stop_after_frames(monkeypatch, 3)
    bus = FakeSMBus([FakeEne(1, led_count=8), FakeEne(3, led_count=8)])
    run(cmd_rainbow, controller, "rainbow", "all", dram=DramController(bus))
    board_frames = [pkt for pkt in transport.sent if pkt[1] == 0x58 and pkt[2:4] == b"\x00\x00"]
    assert len(board_frames) == 3                     # placa: todo quadro (1º pacote de cada)
    for address in (0x70, 0x71):
        frames = memory_frames(bus, address)
        assert len(frames) == 2                       # memórias: quadros 1 e 3
        t = 2 * 5 / 1000                              # fase do 3º quadro (speed 5)
        r, g, b = rainbow_frame(8, t, 1.0)[0]
        assert frames[-1][:3] == [r, b, g]            # mesma fase da placa, ordem R,B,G
    for chip in bus.chips:
        assert chip.regs[ene.REG_DIRECT] == 1         # cores do PC, não o rainbow do chip


def test_animated_rainbow_on_memory_only_also_streams(controller, monkeypatch):
    stop_after_first_frame(monkeypatch)
    bus = FakeSMBus([FakeEne(1)])
    run(cmd_rainbow, controller, "rainbow", "dram", dram=DramController(bus))
    assert len(memory_frames(bus, 0x70)) == 1


# --- perfil e run ---------------------------------------------------------

def test_profile_saves_a_valid_command(capsys):
    run(cmd_profile, None, "profile", "rainbow", "all", "--speed", "3")
    assert Config().profile() == ["rainbow", "all", "--speed", "3"]
    assert "perfil salvo" in capsys.readouterr().out


@pytest.mark.parametrize("argv", [["calibrate", "d_led1"], ["rainbow", "led_c1"], ["color", "all"]])
def test_profile_rejects_invalid_commands(argv):
    with pytest.raises(ValueError):
        run(cmd_profile, None, "profile", *argv)
    assert Config().profile() is None


def test_profile_without_arguments_shows_the_default(capsys):
    run(cmd_profile, None, "profile")
    assert "padrão: rainbow all" in capsys.readouterr().out


@pytest.fixture
def devices_ready(monkeypatch):
    monkeypatch.setattr("aorus_rgb.__main__.wait_for_device_access", list)


def test_run_applies_the_saved_profile(controller, transport, devices_ready):
    Config().set_profile(["color", "d_led1", "ff0000"])
    controller.config = Config()
    run(cmd_run, controller, "run")
    assert [pkt[1] for pkt in transport.sent if pkt[1] in (0x58, 0x59)] == [0x58, 0x58]


def test_run_without_profile_uses_the_rainbow(controller, transport, monkeypatch, devices_ready):
    stop_after_first_frame(monkeypatch)
    run(cmd_run, controller, "run")
    assert {pkt[1] for pkt in transport.sent} >= {0x58, 0x59}


# --- espera pela permissão no login ---------------------------------------

class FakeClock:
    """Relógio simulado: cada sleep avança o tempo, sem esperar de verdade."""

    def __init__(self):
        self.now = 0.0
        self.sleeps = 0

    def monotonic(self):
        return self.now

    def sleep(self, seconds):
        self.sleeps += 1
        self.now += seconds


def fake_devices(monkeypatch, board="/dev/hidraw5", bus="/dev/i2c-0", granted_after=0):
    clock = FakeClock()
    monkeypatch.setattr("aorus_rgb.__main__.time", clock)

    def find_board():
        if board is None:
            raise DeviceNotFound("sem placa")
        return board

    def find_bus():
        if bus is None:
            raise BusNotFound("sem SMBus")
        return bus

    monkeypatch.setattr("aorus_rgb.__main__.find_hidraw", find_board)
    monkeypatch.setattr("aorus_rgb.__main__.find_dram_bus", find_bus)
    monkeypatch.setattr("aorus_rgb.__main__.os.access", lambda path, mode: clock.sleeps >= granted_after)
    return clock


def test_wait_returns_as_soon_as_access_is_granted(monkeypatch):
    clock = fake_devices(monkeypatch, granted_after=3)
    assert wait_for_device_access() == []
    assert clock.sleeps == 3


def test_wait_gives_up_after_the_timeout(monkeypatch):
    clock = fake_devices(monkeypatch, granted_after=10_000)
    assert wait_for_device_access(timeout=2) == ["/dev/hidraw5", "/dev/i2c-0"]
    assert clock.now >= 2


def test_wait_ignores_a_missing_smbus(monkeypatch):
    fake_devices(monkeypatch, bus=None)
    assert wait_for_device_access() == []


def test_wait_keeps_waiting_for_a_board_not_yet_detected(monkeypatch):
    fake_devices(monkeypatch, board=None)
    assert wait_for_device_access(timeout=1) == ["placa (ainda não detectada)"]
