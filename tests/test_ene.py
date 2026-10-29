import pytest
from fake_smbus import FakeEne, FakeSMBus

from aorus_rgb import ene
from aorus_rgb.dram import DramController, ene_speed
from aorus_rgb.smbus import ForbiddenAddress, SMBus, check_address


def two_sticks():
    # pentes nos slots A2 e B2 (índices 1 e 3), os dois ainda em 0x77
    return FakeSMBus([FakeEne(1), FakeEne(3)])


def test_remap_gives_each_stick_its_own_address():
    bus = two_sticks()
    assert ene.remap(bus) == [0x70, 0x71]
    assert [c.address for c in bus.chips] == [0x70, 0x71]
    assert not ene.present(bus, ene.DEFAULT_ADDRESS)


def test_remap_skips_occupied_addresses():
    bus = FakeSMBus([FakeEne(0)], others={0x70})
    assert ene.remap(bus) == [0x71]


def test_remap_is_noop_when_already_done():
    bus = two_sticks()
    ene.remap(bus)
    bus.log.clear()
    assert ene.remap(bus) == []
    assert bus.writes() == []


def test_remap_does_not_write_to_non_ene_device():
    bus = FakeSMBus(others={0x77})
    assert ene.remap(bus) == []
    assert bus.log == []


def test_discover_reads_version_and_led_count():
    bus = FakeSMBus([FakeEne(1, led_count=8), FakeEne(3, led_count=8)])
    devices = ene.discover(bus)
    assert [(d.address, d.version, d.led_count) for d in devices] == [
        (0x70, "AUDA0-E6K5-0101", 8),
        (0x71, "AUDA0-E6K5-0101", 8),
    ]


def test_discover_skips_unsupported_versions():
    bus = FakeSMBus([FakeEne(0, version=b"DIMM_LED-0102\0")])
    assert ene.discover(bus) == []


def test_direct_colors_use_rbg_order_in_3_byte_blocks():
    bus = FakeSMBus([FakeEne(0, led_count=2)])
    (dev,) = ene.discover(bus)
    dev.set_colors([(1, 2, 3), (4, 5, 6)])
    assert bus.chips[0].colors(0x8100, 2) == [1, 3, 2, 4, 6, 5]
    blocks = [e for e in bus.log if e[0] == "block"]
    assert [len(e[3]) for e in blocks] == [3, 3]


def test_block_write_falls_back_to_bytes():
    bus = FakeSMBus([FakeEne(0, led_count=1)])
    bus.block_supported = False
    (dev,) = ene.discover(bus)
    dev.set_colors([(9, 8, 7)])
    assert bus.chips[0].colors(0x8100, 1) == [9, 7, 8]


def test_set_colors_rejects_wrong_count():
    (dev,) = ene.discover(FakeSMBus([FakeEne(0, led_count=5)]))
    with pytest.raises(ValueError):
        dev.set_colors([(0, 0, 0)])


def test_set_mode_writes_effect_colors_and_registers():
    bus = FakeSMBus([FakeEne(0, led_count=1)])
    (dev,) = ene.discover(bus)
    dev.set_mode("breathing", [(255, 0, 16)], speed=4)
    regs = bus.chips[0].regs
    assert bus.chips[0].colors(0x8160, 1) == [255, 16, 0]
    assert regs[ene.REG_DIRECT] == 0
    assert regs[ene.REG_MODE] == ene.MODES["breathing"]
    assert regs[ene.REG_SPEED] == 4
    assert regs[ene.REG_APPLY] == ene.APPLY_VAL


def test_set_mode_validates():
    (dev,) = ene.discover(FakeSMBus([FakeEne(0, led_count=1)]))
    with pytest.raises(ValueError):
        dev.set_mode("disco", [(0, 0, 0)])
    with pytest.raises(ValueError):
        dev.set_mode("static", [(0, 0, 0)], speed=5)


@pytest.mark.parametrize("speed, expected", [(0, 0), (2, 1), (5, 2), (9, 4)])
def test_ene_speed_mapping(speed, expected):
    assert ene_speed(speed) == expected


def test_dram_controller_names_zones_and_enables_direct_once():
    bus = two_sticks()
    dram = DramController(bus)
    assert list(dram.zones) == ["dram1", "dram2"]
    z = dram.zones["dram1"]
    dram.set_color(z, (255, 0, 0))
    dram.set_color(z, (0, 255, 0))
    direct_writes = [e for e in bus.log if e == ("word", 0x70, 0x00, 0x2080)]
    assert len(direct_writes) == 1
    assert bus.chips[0].colors(0x8100, 1) == [0, 0, 255]


def test_dram_effect_maps_names_and_brightness():
    bus = FakeSMBus([FakeEne(0, led_count=1)])
    dram = DramController(bus)
    dram.set_effect(dram.zones["dram1"], "pulse", (255, 255, 255), brightness=51, speed=9)
    regs = bus.chips[0].regs
    assert regs[ene.REG_MODE] == ene.MODES["breathing"]
    assert regs[ene.REG_SPEED] == 4
    assert bus.chips[0].colors(0x8160, 1) == [51, 51, 51]


@pytest.mark.parametrize("address", [0x30, 0x37, 0x50, 0x57])
def test_spd_addresses_are_blocked(address):
    with pytest.raises(ForbiddenAddress):
        check_address(address)


def test_smbus_blocks_spd_before_touching_the_bus():
    bus = SMBus.__new__(SMBus)
    bus.fd, bus._address = -1, None  # um ioctl aqui falharia com EBADF
    with pytest.raises(ForbiddenAddress):
        bus.read_byte(0x50)
