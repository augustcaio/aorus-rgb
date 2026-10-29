import argparse

import pytest

from aorus_rgb.__main__ import build_parser, parse_color


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
