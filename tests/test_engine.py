import pytest
from fake_smbus import FakeEne, FakeSMBus

from aorus_rgb import ene
from aorus_rgb.config import MemoryConfig
from aorus_rgb.dram import DramController
from aorus_rgb.engine import Engine, parse_hex
from aorus_rgb.patterns import rainbow_frame


class FakeClock:
    def __init__(self):
        self.now = 100.0

    def __call__(self):
        return self.now


@pytest.fixture
def bus():
    return FakeSMBus([FakeEne(1, led_count=8), FakeEne(3, led_count=8)])


@pytest.fixture
def clock():
    return FakeClock()


@pytest.fixture
def engine(controller, bus, clock):
    config = MemoryConfig()
    controller.config = config
    return Engine(controller, DramController(bus), config, clock=clock)


def board_frames(transport, reg):
    """Primeiro pacote de cada quadro enviado a um header ARGB."""
    return [pkt for pkt in transport.sent if pkt[1] == reg and pkt[2:4] == b"\x00\x00"]


def led1_grb_to_rgb(pkt):
    g, r, b = pkt[5:8]
    return r, g, b


def dram_led1(bus, index):
    r, b, g = bus.chips[index].colors(0x8100, 1)
    return r, g, b


# --- consulta ----------------------------------------------------------------

def test_zone_info_lists_board_and_memory(engine):
    assert engine.zone_info() == [
        ("d_led1", "argb", 32), ("d_led2", "argb", 32),
        ("led_c1", "rgb", 1), ("led_cpu", "rgb", 1), ("led_c2", "rgb", 1),
        ("dram1", "dram", 8), ("dram2", "dram", 8),
    ]


def test_zone_info_uses_the_calibrated_led_count(engine):
    engine.config.set_leds("d_led1", 12)
    assert ("d_led1", "argb", 12) in engine.zone_info()


def test_works_without_memory(controller):
    engine = Engine(controller, None, MemoryConfig())
    assert [n for n, _, _ in engine.zone_info()] == ["d_led1", "d_led2", "led_c1", "led_cpu", "led_c2"]
    assert engine.resolve("dram") == []


def test_device_info(engine):
    info = engine.device_info()
    assert info["board.product"].startswith("IT5702")
    assert info["dram1.controller"] == "AUDA0-E6K5-0101 em 0x70"


def test_resolve_groups_and_unknown(engine):
    assert engine.resolve("dram") == ["dram1", "dram2"]
    assert len(engine.resolve("all")) == 7
    with pytest.raises(ValueError, match="dram9"):
        engine.resolve("dram9")


# --- comandos ----------------------------------------------------------------

def test_set_color_updates_hardware_scene_and_config(engine, transport, bus):
    engine.set_color("all", "#FF0000")
    assert {s["mode"] for s in engine.scene.values()} == {"color"}
    assert engine.scene["dram1"]["color"] == "ff0000"
    assert engine.config.scene() == engine.scene
    assert led1_grb_to_rgb(board_frames(transport, 0x58)[-1]) == (255, 0, 0)
    assert dram_led1(bus, 0) == (255, 0, 0)


@pytest.mark.parametrize(
    "args",
    [("d_led1", "disco", "ff0000"), ("d_led1", "pulse", "zz0000"), ("d_led1", "pulse", "ff0000", 300)],
)
def test_set_effect_validates(engine, args):
    with pytest.raises(ValueError):
        engine.set_effect(*args)


def test_set_effect_records_parameters(engine):
    engine.set_effect("led_c1", "pulse", "00ffff", 200, 3)
    assert engine.scene["led_c1"] == {
        "mode": "effect", "effect": "pulse", "color": "00ffff", "brightness": 200, "speed": 3,
    }


def test_rainbow_skips_single_color_zones_in_groups(engine):
    engine.set_rainbow("all")
    assert set(engine.scene) == {"d_led1", "d_led2", "dram1", "dram2"}
    assert engine.animated == ["d_led1", "d_led2", "dram1", "dram2"]


def test_rainbow_on_a_single_color_zone_is_an_error(engine):
    with pytest.raises(ValueError, match="endereçável"):
        engine.set_rainbow("led_c1")


def test_static_rainbow_is_painted_once_and_not_animated(engine, transport):
    engine.set_rainbow("d_led1", speed=0)
    assert engine.animated == []
    assert led1_grb_to_rgb(board_frames(transport, 0x58)[-1]) == (255, 0, 0)


def test_off_and_new_mode_stop_the_animation(engine):
    engine.set_rainbow("all")
    engine.off("dram1")
    engine.set_color("d_led2", "00ff00")
    assert engine.animated == ["d_led1", "dram2"]
    assert engine.scene["dram1"] == {"mode": "off"}


def test_listeners_are_notified(engine):
    calls = []
    engine.listeners.append(lambda: calls.append(1))
    engine.set_color("d_led1", "ff0000")
    engine.save_profile("x")
    assert len(calls) == 2


# --- animação ----------------------------------------------------------------

def test_zones_with_the_same_speed_stay_in_phase(engine, transport, bus, clock):
    engine.set_rainbow("d_led1", speed=5)
    clock.now += 7.3                       # dram1 ligado bem depois
    engine.set_rainbow("dram1", speed=5)
    transport.sent.clear()
    clock.now += 1.1
    engine.tick()                          # quadro par: placa e memórias
    expected = rainbow_frame(1, engine.phase(5), 1.0)[0]
    assert led1_grb_to_rgb(board_frames(transport, 0x58)[-1]) == expected
    assert dram_led1(bus, 0) == expected


def test_memory_gets_every_second_frame(engine, transport, bus):
    engine.set_rainbow("all")
    transport.sent.clear()
    bus.log.clear()
    for _ in range(4):
        engine.tick()
    assert len(board_frames(transport, 0x58)) == 4
    assert len([e for e in bus.log if e[:3] == ("block", 0x70, 0x03)]) == 2


def test_phase_follows_the_clock(engine, clock):
    assert engine.phase(5) == 0
    clock.now += 10
    assert engine.phase(5) == pytest.approx((10 * 5 * 0.03) % 1)


# --- cena, restauração e perfis ---------------------------------------------

def test_first_run_restores_a_rainbow_on_addressable_zones(engine):
    engine.restore()
    assert set(engine.scene) == {"d_led1", "d_led2", "dram1", "dram2"}
    assert all(s["mode"] == "rainbow" for s in engine.scene.values())


def test_restore_applies_the_saved_scene(engine):
    engine.config.set_scene({"d_led1": {"mode": "color", "color": "123456"}, "dram7": {"mode": "off"}})
    engine.restore()
    assert engine.scene == {"d_led1": {"mode": "color", "color": "123456"}}


def test_profiles_roundtrip(engine):
    engine.set_color("all", "ff0000")
    engine.save_profile("vermelho")
    engine.set_rainbow("all")
    assert engine.list_profiles() == ["vermelho"]
    engine.apply_profile("vermelho")
    assert engine.scene["d_led1"] == {"mode": "color", "color": "ff0000"}
    engine.delete_profile("vermelho")
    assert engine.list_profiles() == []


@pytest.mark.parametrize("name", ["", "   ", "x" * 65])
def test_profile_names_are_validated(engine, name):
    with pytest.raises(ValueError):
        engine.save_profile(name)


def test_unknown_profile(engine):
    with pytest.raises(ValueError):
        engine.apply_profile("nada")
    with pytest.raises(ValueError):
        engine.delete_profile("nada")


# --- calibração --------------------------------------------------------------

def test_calibration_pauses_the_zone_and_resumes_it(engine, transport):
    engine.set_rainbow("all")
    engine.light_first("d_led1", 10)
    assert "d_led1" not in engine.animated
    engine.set_led_count("d_led1", 12)
    assert engine.config.leds("d_led1") == 12
    assert engine.board.led_count(engine.zones["d_led1"][1]) == 12
    assert "d_led1" in engine.animated


def test_end_calibration_without_saving(engine):
    engine.set_rainbow("d_led1")
    engine.light_first("d_led1", 5)
    engine.end_calibration("d_led1")
    assert engine.animated == ["d_led1"]
    assert engine.config.leds("d_led1") is None


@pytest.mark.parametrize("name", ["led_c1", "dram1", "nada"])
def test_calibration_only_on_argb_headers(engine, name):
    with pytest.raises(ValueError, match="ARGB"):
        engine.light_first(name, 4)


def test_calibration_count_range(engine):
    with pytest.raises(ValueError):
        engine.light_first("d_led1", 0)
    with pytest.raises(ValueError):
        engine.set_led_count("d_led1", 300)


def test_parse_hex():
    assert parse_hex("#00ff80") == (0, 255, 128)
    with pytest.raises(ValueError):
        parse_hex("fff")


def test_memory_mode_registers_on_effect(engine, bus):
    engine.set_effect("dram", "pulse", "00ff00")
    assert bus.chips[0].regs[ene.REG_MODE] == ene.MODES["breathing"]
