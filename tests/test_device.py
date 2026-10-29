import json

from aorus_rgb import protocol as p
from aorus_rgb.device import Controller, MemoryState, State

D_LED1 = p.ZONES["d_led1"]
D_LED2 = p.ZONES["d_led2"]
LED_C1 = p.ZONES["led_c1"]


def masks(transport):
    return [pkt[2] for pkt in transport.sent if pkt[1] == p.CMD_BUILTIN_EFFECT_OFF]


def test_init_reads_info_and_initializes_board(transport):
    c = Controller(transport, MemoryState())
    assert transport.commands() == [p.CMD_INFO, *range(0x20, 0x28), p.CMD_APPLY, p.CMD_BEAT]
    assert transport.sent[-1][2] == 0  # beat desligado
    assert c.info.product.startswith("IT5702")


def test_board_is_initialized_once_per_boot(transport):
    state = MemoryState()
    Controller(transport, state)
    transport.sent.clear()
    Controller(transport, state)
    assert transport.commands() == [p.CMD_INFO]


def test_initialized_flag_survives_runs_but_not_reboots(tmp_path, monkeypatch, transport):
    path = tmp_path / "state.json"
    monkeypatch.setattr("aorus_rgb.device._boot_id", lambda: "boot-a")
    Controller(transport, State(path))
    assert State(path).board_initialized
    monkeypatch.setattr("aorus_rgb.device._boot_id", lambda: "boot-b")
    assert not State(path).board_initialized


def test_color_on_second_header_keeps_first_disabled(controller, transport):
    """Regressão: d_led2 não pode religar o efeito embutido do d_led1."""
    controller.set_color(D_LED1, (255, 0, 0))
    controller.set_color(D_LED2, (0, 0, 255))
    assert masks(transport) == [0x01, 0x03]


def test_direct_colors_skip_mask_when_already_disabled(controller, transport):
    controller.set_color(D_LED1, (255, 0, 0))
    transport.sent.clear()
    controller.set_color(D_LED1, (0, 255, 0))
    assert masks(transport) == []
    assert transport.commands() == [0x58, 0x58]  # 32 LEDs = 2 pacotes


def test_hardware_effect_reenables_builtin_only_for_its_header(controller, transport):
    controller.set_color(D_LED1, (255, 0, 0))
    controller.set_color(D_LED2, (255, 0, 0))
    transport.sent.clear()
    controller.set_effect(D_LED2, "pulse", (0, 255, 0))
    assert masks(transport) == [0x01]
    assert transport.commands() == [p.CMD_BUILTIN_EFFECT_OFF, 0x26, p.CMD_APPLY]


def test_color_on_rgb_header_uses_static_effect(controller, transport):
    controller.set_color(LED_C1, (0, 0, 255))
    assert transport.commands() == [0x21, p.CMD_APPLY]
    assert transport.sent[0][11] == p.EFFECTS["static"]


def test_off_rgb_header_uses_effect_none(controller, transport):
    controller.off(LED_C1)
    assert transport.sent[0][11] == p.EFFECTS["none"]


def test_set_led_count_only_sends_on_change(controller, transport):
    controller.set_led_count(D_LED1, 20)
    assert transport.sent == []
    controller.set_led_count(D_LED1, 60)
    assert transport.commands() == [p.CMD_LED_COUNT]
    assert controller.led_count(D_LED1) == 64


def test_state_persists_within_same_boot(tmp_path, monkeypatch):
    monkeypatch.setattr("aorus_rgb.device._boot_id", lambda: "boot-a")
    path = tmp_path / "state.json"
    s = State(path)
    s.builtin_off_mask = 0x03
    s.save()
    assert State(path).builtin_off_mask == 0x03


def test_state_is_discarded_after_reboot(tmp_path, monkeypatch):
    path = tmp_path / "state.json"
    path.write_text(json.dumps({"boot_id": "boot-a", "builtin_off_mask": 3}))
    monkeypatch.setattr("aorus_rgb.device._boot_id", lambda: "boot-b")
    assert State(path).builtin_off_mask == 0


def test_state_ignores_corrupt_file(tmp_path):
    path = tmp_path / "state.json"
    path.write_text("{oops")
    assert State(path).builtin_off_mask == 0
