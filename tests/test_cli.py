import argparse

import pytest

from aorus_rgb import protocol as p
from aorus_rgb.__main__ import build_parser, cmd_calibrate, cmd_count, cmd_rainbow, parse_color, rainbow_frame


def test_parse_color():
    assert parse_color("ff8000") == (255, 128, 0)
    assert parse_color("#00ff00") == (0, 255, 0)


@pytest.mark.parametrize("bad", ["fff", "gg0000", "ff00001"])
def test_parse_color_rejects_invalid(bad):
    with pytest.raises(argparse.ArgumentTypeError):
        parse_color(bad)


def test_rainbow_only_accepts_argb_zones():
    with pytest.raises(SystemExit):
        build_parser().parse_args(["rainbow", "led_c1"])


def test_off_defaults_to_all():
    assert build_parser().parse_args(["off"]).zone == "all"


def test_rainbow_accepts_all():
    assert build_parser().parse_args(["rainbow", "all"]).zone == "all"


def test_effect_no_longer_offers_hardware_wave():
    with pytest.raises(SystemExit):
        build_parser().parse_args(["effect", "all", "wave", "ff0000"])


def test_rainbow_frame_spans_the_hue_circle():
    frame = rainbow_frame(3, 0.0, 1.0)
    assert frame == [(255, 0, 0), (0, 255, 0), (0, 0, 255)]


def test_static_rainbow_paints_all_argb_headers_once(controller, transport):
    a = build_parser().parse_args(["rainbow", "all", "--speed", "0"])
    cmd_rainbow(controller, a)  # retorna sem entrar no loop
    argb = [pkt for pkt in transport.sent if pkt[1] in (0x58, 0x59)]
    assert [pkt[1] for pkt in argb] == [0x58, 0x58, 0x59, 0x59]  # 32 LEDs = 2 pacotes por header
    # primeiro LED vermelho (ordem GRB no fio)
    assert argb[0][5:8] == bytes([0, 255, 0])


def test_count_lights_first_n_white_and_clears_the_rest(controller, transport):
    cmd_count(controller, build_parser().parse_args(["count", "d_led1", "20"]))
    data = b"".join(pkt[5:5 + pkt[4]] for pkt in transport.sent if pkt[1] == 0x58)
    assert len(data) == 32 * 3
    assert data[:20 * 3] == b"\xff" * 60
    assert data[20 * 3:] == bytes(12 * 3)


def test_count_grows_the_strip_when_needed(controller, transport):
    cmd_count(controller, build_parser().parse_args(["count", "d_led1", "40"]))
    assert controller.led_count(p.ZONES["d_led1"]) == 64


def test_count_rejects_out_of_range():
    with pytest.raises(SystemExit):
        build_parser().parse_args(["count", "d_led1", "0"])


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


@pytest.mark.parametrize("physical", [1, 9, 54, 256])
def test_calibrate_finds_and_saves_the_led_count(controller, transport, monkeypatch, physical):
    questions = answer_like_a_strip_of(physical, transport, monkeypatch)
    cmd_calibrate(controller, build_parser().parse_args(["calibrate", "d_led1"]))
    assert controller.config.leds("d_led1") == physical
    assert controller.led_count(p.ZONES["d_led1"]) == physical
    assert len(questions) <= 9  # 1 conferência + busca binária em 256


def test_calibrate_stops_when_256_is_not_enough(controller, transport, monkeypatch):
    monkeypatch.setattr("builtins.input", lambda prompt: "n")
    with pytest.raises(ValueError, match="256"):
        cmd_calibrate(controller, build_parser().parse_args(["calibrate", "d_led1"]))
    assert controller.config.leds("d_led1") is None


def test_calibrate_needs_an_interactive_terminal(controller, monkeypatch):
    def no_stdin(prompt):
        raise EOFError

    monkeypatch.setattr("builtins.input", no_stdin)
    with pytest.raises(ValueError, match="terminal"):
        cmd_calibrate(controller, build_parser().parse_args(["calibrate", "d_led1"]))
