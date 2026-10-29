import struct

import pytest
from conftest import REAL_INFO

from aorus_rgb import protocol as p

D_LED1 = p.ZONES["d_led1"]
D_LED2 = p.ZONES["d_led2"]
LED_C1 = p.ZONES["led_c1"]


def test_command_is_padded_feature_report():
    pkt = p.command(0x28, 0xFF)
    assert len(pkt) == 64
    assert pkt[:3] == b"\xcc\x28\xff"
    assert pkt[3:] == bytes(61)


def test_parse_real_info():
    info = p.parse_info(REAL_INFO)
    assert info.product == "IT5702-GIGABYTE V2.0.10.0"
    assert info.firmware == "2.0.10.0"
    assert info.strip_detect == 0x07
    assert info.support_flags == 0
    assert info.argb_counts == (32,) * 6
    assert info.calibration == (0x00010002, 0x00010002)
    assert info.chip_id == 0x57020100


def test_parse_info_rejects_garbage():
    with pytest.raises(ValueError):
        p.parse_info(bytes(64))


@pytest.mark.parametrize(
    "cal, name",
    [(0x00010002, "GRB"), (0x00000102, "RGB"), (0x00020100, "BGR"), (0x00020001, "GBR")],
)
def test_color_order_name(cal, name):
    assert p.color_order_name(cal) == name


def test_color_order_name_invalid():
    assert p.color_order_name(0x00000000) == "0x000000"


@pytest.mark.parametrize("count, code", [(1, 0), (32, 0), (33, 1), (64, 1), (65, 2), (300, 3), (5000, 4)])
def test_led_count_code(count, code):
    assert p.led_count_code(count) == code


def test_led_count_packet_nibbles():
    pkt = p.led_count_packet([64, 256, 32, 32, 32, 32])
    assert pkt[1:5] == bytes([0x34, 0x21, 0x00, 0x00])


def test_static_effect_layout():
    """Offsets da struct PktEffect do OpenRGB."""
    pkt = p.effect_packet(D_LED1, "static", (0xFF, 0x10, 0x01), brightness=200)
    assert pkt[1] == 0x25                                   # 0x20 + índice 5
    assert struct.unpack_from("<II", pkt, 2) == (1 << 5, 0)  # zone0, zone1
    assert pkt[11:14] == bytes([1, 200, 0])                 # tipo, brilho máx/mín
    assert struct.unpack_from("<I", pkt, 14)[0] == 0xFF1001  # cor 0x00RRGGBB
    assert pkt[22:34] == bytes(12)                          # sem períodos/params


def test_effect_on_rgb_header():
    pkt = p.effect_packet(LED_C1, "static", (0, 0, 255))
    assert pkt[1] == 0x21
    assert struct.unpack_from("<I", pkt, 2)[0] == 0b10


@pytest.mark.parametrize(
    "effect, speed, periods, params",
    [
        ("pulse", 0, (400, 400, 200, 0), (0, 0, 0, 0)),
        ("pulse", 6, (1000, 1000, 200, 0), (0, 0, 0, 0)),
        ("pulse", 9, (1600, 1600, 200, 0), (0, 0, 0, 0)),
        ("flash", 5, (100, 100, 1700, 0), (0, 0, 0, 0)),
        ("cycle", 5, (800, 600, 0, 0), (7, 0, 0, 0)),
        ("cycle", 9, (2500, 2300, 0, 0), (7, 0, 0, 0)),
        ("wave", 0, (30, 0, 0, 0), (7, 1, 0, 0)),
        ("wave", 9, (300, 0, 0, 0), (7, 1, 0, 0)),
        ("random", 5, (100, 0, 0, 0), (1, 5, 0, 0)),
    ],
)
def test_effect_timings_match_gigabyte_ranges(effect, speed, periods, params):
    pkt = p.effect_packet(D_LED1, effect, (255, 0, 0), speed=speed)
    assert pkt[11] == p.EFFECTS[effect]
    assert struct.unpack_from("<4H", pkt, 22) == periods
    assert tuple(pkt[30:34]) == params


@pytest.mark.parametrize("kwargs", [{"speed": 10}, {"speed": -1}, {"brightness": 256}])
def test_effect_rejects_out_of_range(kwargs):
    with pytest.raises(ValueError):
        p.effect_packet(D_LED1, "static", (0, 0, 0), **kwargs)


def test_effect_rejects_unknown_name():
    with pytest.raises(ValueError):
        p.effect_packet(D_LED1, "disco", (0, 0, 0))


def test_argb_packets_split_and_offsets():
    colors = [(i, 0, 0) for i in range(20)]
    first, second = p.argb_packets(D_LED2, colors, 0x00000102)
    assert first[1] == 0x59
    assert struct.unpack_from("<H", first, 2)[0] == 0 and first[4] == 19 * 3
    assert struct.unpack_from("<H", second, 2)[0] == 19 * 3 and second[4] == 3
    assert second[5:8] == bytes([19, 0, 0])
    assert second[8:] == bytes(56)


def test_argb_packets_apply_grb_order():
    (pkt,) = p.argb_packets(D_LED1, [(0xAA, 0xBB, 0xCC)], 0x00010002)
    assert pkt[5:8] == bytes([0xBB, 0xAA, 0xCC])


def test_argb_packets_reject_rgb_header():
    with pytest.raises(ValueError):
        p.argb_packets(LED_C1, [(1, 2, 3)], 0)
