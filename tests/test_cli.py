import argparse

import pytest

from aorus_rgb import protocol as p
from aorus_rgb.__main__ import build_parser, cmd_count, cmd_rainbow, parse_color, rainbow_frame


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
