import argparse
from contextlib import ExitStack

import pytest
from fake_smbus import FakeEne, FakeSMBus

from aorus_rgb.__main__ import Devices, build_parser, parse_color, resolve
from aorus_rgb.dram import DramController


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


def test_rainbow_accepts_dram():
    assert build_parser().parse_args(["rainbow", "dram1"]).zone == "dram1"


def test_off_defaults_to_all():
    assert build_parser().parse_args(["off"]).zone == "all"


def devices(board, dram=None):
    def no_access():
        raise PermissionError(13, "Permission denied", "/dev/i2c-0")

    return Devices(ExitStack(), board_factory=lambda: board, dram_factory=(lambda: dram) if dram else no_access)


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
