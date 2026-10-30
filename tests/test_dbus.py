"""Testes do daemon pelo D-Bus, num barramento privado (Gio.TestDBus)."""

import re
import threading
import time

import pytest
from fake_smbus import FakeEne, FakeSMBus

from aorus_rgb.config import MemoryConfig
from aorus_rgb.dram import DramController
from aorus_rgb.engine import Engine

gi = pytest.importorskip("gi")
gi.require_version("Gio", "2.0")
from gi.repository import Gio, GLib

from aorus_rgb.dbus_service import BUS_NAME, ERROR, INTERFACE, OBJECT_PATH, Service

FLAGS = Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION


@pytest.fixture
def engine(controller):
    config = MemoryConfig()
    controller.config = config
    bus = FakeSMBus([FakeEne(1, led_count=8), FakeEne(3, led_count=8)])
    return Engine(controller, DramController(bus), config)


@pytest.fixture
def daemon(engine):
    """Serviço numa thread com o próprio laço GLib; devolve a conexão do cliente."""
    test_bus = Gio.TestDBus.new(Gio.TestDBusFlags.NONE)
    test_bus.up()
    address = test_bus.get_bus_address()
    ready = threading.Event()
    server = {}

    def serve():
        context = GLib.MainContext.new()
        context.push_thread_default()
        loop = GLib.MainLoop.new(context, False)
        connection = Gio.DBusConnection.new_for_address_sync(address, FLAGS, None, None)
        service = Service(engine)
        service.register(connection)
        connection.call_sync(
            "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "RequestName",
            GLib.Variant("(su)", (BUS_NAME, 0)), None, Gio.DBusCallFlags.NONE, 5000, None,
        )
        server.update(loop=loop, service=service)
        ready.set()
        loop.run()
        context.pop_thread_default()
        connection.close_sync(None)

    thread = threading.Thread(target=serve, daemon=True)
    thread.start()
    assert ready.wait(5)
    client = Gio.DBusConnection.new_for_address_sync(address, FLAGS, None, None)
    yield client, server["service"]
    server["loop"].quit()
    thread.join(5)
    client.close_sync(None)
    test_bus.down()


def call(client, method, signature=None, *args):
    params = GLib.Variant(signature, args) if signature else None
    reply = client.call_sync(BUS_NAME, OBJECT_PATH, INTERFACE, method, params, None,
                             Gio.DBusCallFlags.NONE, 5000, None)
    return reply.unpack() if reply is not None else ()


def remote_error(excinfo):
    return Gio.DBusError.get_remote_error(excinfo.value)


def test_get_zones(daemon):
    client, _ = daemon
    (zones,) = call(client, "GetZones")
    assert zones[0] == ("d_led1", "argb", 32)
    assert ("dram2", "dram", 8) in zones


def test_set_color_and_get_state(daemon):
    client, _ = daemon
    call(client, "SetColor", "(ss)", "dram", "#00FF00")
    (state,) = call(client, "GetState")
    assert state["dram1"] == {"mode": "color", "color": "00ff00"}


def test_effect_state_keeps_types(daemon):
    client, _ = daemon
    call(client, "SetEffect", "(sssuu)", "led_c1", "pulse", "ff00ff", 200, 3)
    (state,) = call(client, "GetState")
    assert state["led_c1"] == {"mode": "effect", "effect": "pulse", "color": "ff00ff", "brightness": 200, "speed": 3}


def test_errors_become_dbus_errors(daemon):
    client, _ = daemon
    with pytest.raises(GLib.Error) as excinfo:
        call(client, "SetColor", "(ss)", "d_led9", "ff0000")
    assert remote_error(excinfo) == ERROR
    assert "d_led9" in excinfo.value.message


def test_rainbow_starts_and_off_stops_the_animation(daemon, engine):
    client, service = daemon
    call(client, "SetRainbow", "(sdu)", "all", 5.0, 255)
    deadline = time.monotonic() + 2
    while engine.frame < 3 and time.monotonic() < deadline:
        time.sleep(0.02)
    assert engine.frame >= 3  # o timer está animando
    call(client, "Off", "(s)", "all")
    assert service.timer is None


def test_profiles(daemon):
    client, _ = daemon
    call(client, "SetColor", "(ss)", "all", "123456")
    call(client, "SaveProfile", "(s)", "azul")
    assert call(client, "ListProfiles") == (["azul"],)
    call(client, "DeleteProfile", "(s)", "azul")
    assert call(client, "ListProfiles") == ([],)


def test_calibration_methods(daemon, engine):
    client, _ = daemon
    call(client, "LightFirst", "(su)", "d_led1", 12)
    assert "d_led1" in engine.calibrating
    call(client, "SetLedCount", "(su)", "d_led1", 12)
    assert engine.config.leds("d_led1") == 12
    assert "d_led1" not in engine.calibrating


def test_device_info_and_version(daemon):
    client, _ = daemon
    (info,) = call(client, "GetDeviceInfo")
    assert info["board.product"].startswith("IT5702")
    reply = client.call_sync(BUS_NAME, OBJECT_PATH, "org.freedesktop.DBus.Properties", "Get",
                             GLib.Variant("(ss)", (INTERFACE, "Version")), None,
                             Gio.DBusCallFlags.NONE, 5000, None)
    assert isinstance(reply.unpack()[0], str)


def test_state_changed_signal(daemon):
    client, _ = daemon
    received = []
    context = GLib.MainContext.new()
    context.push_thread_default()
    try:
        client.signal_subscribe(BUS_NAME, INTERFACE, "StateChanged", OBJECT_PATH, None,
                                Gio.DBusSignalFlags.NONE, lambda *args: received.append(args[4]))
        call(client, "SetColor", "(ss)", "d_led1", "ff0000")
        deadline = time.monotonic() + 2
        while not received and time.monotonic() < deadline:
            context.iteration(False)
            time.sleep(0.01)
    finally:
        context.pop_thread_default()
    assert received == ["StateChanged"]


def test_introspection_lists_the_api(daemon):
    client, _ = daemon
    reply = client.call_sync(BUS_NAME, OBJECT_PATH, "org.freedesktop.DBus.Introspectable", "Introspect",
                             None, None, Gio.DBusCallFlags.NONE, 5000, None)
    xml = reply.unpack()[0]
    for method in ("SetRainbow", "ApplyProfile", "LightFirst"):
        assert f'name="{method}"' in xml


# --- CLI pelo daemon ----------------------------------------------------------

@pytest.fixture
def cli_daemon(daemon, monkeypatch):
    """CLI apontada para o daemon do barramento privado; devolve o motor."""
    from aorus_rgb import __main__ as cli
    from aorus_rgb.client import DaemonClient

    client, service = daemon
    monkeypatch.setattr(cli, "connect", lambda: DaemonClient(client))
    return cli, service.engine


def test_cli_commands_go_through_the_daemon(cli_daemon):
    cli, engine = cli_daemon
    cli.main(["color", "d_led1", "#FF0000"])
    cli.main(["effect", "led_c1", "pulse", "00ffff", "--speed", "3"])
    cli.main(["rainbow", "dram", "--speed", "0"])
    cli.main(["off", "led_c2"])
    assert engine.scene["d_led1"] == {"mode": "color", "color": "ff0000"}
    assert engine.scene["led_c1"]["speed"] == 3
    assert engine.scene["dram2"] == {"mode": "rainbow", "speed": 0.0, "brightness": 255}
    assert engine.scene["led_c2"] == {"mode": "off"}


def test_cli_shows_daemon_errors(cli_daemon):
    cli, _ = cli_daemon
    with pytest.raises(SystemExit) as excinfo:
        cli.main(["color", "dram3", "ff0000"])
    assert str(excinfo.value) == "erro: zona desconhecida: dram3"  # sem o prefixo GDBus.Error


def test_cli_leds_and_count_need_the_daemon_stopped(cli_daemon):
    cli, _ = cli_daemon
    with pytest.raises(SystemExit, match="--leds"):
        cli.main(["color", "d_led1", "ff0000", "--leds", "20"])
    with pytest.raises(SystemExit, match="check d_led1 --leds 12"):
        cli.main(["count", "d_led1", "12"])


def test_cli_info_through_the_daemon(cli_daemon, capsys):
    cli, _ = cli_daemon
    cli.main(["color", "dram1", "00ff00"])
    cli.main(["info"])
    out = capsys.readouterr().out
    assert "daemon      : ativo" in out
    assert "dram1    DRAM    8 LEDs  cor #00ff00" in out
    assert "led_c1   RGB     1 LED " in out


def test_cli_profiles_through_the_daemon(cli_daemon, capsys):
    cli, engine = cli_daemon
    cli.main(["color", "all", "ff0000"])
    cli.main(["profile", "save", "vermelho"])
    cli.main(["profile"])
    assert "vermelho" in capsys.readouterr().out
    cli.main(["rainbow", "all"])
    cli.main(["profile", "apply", "vermelho"])
    assert engine.scene["d_led1"] == {"mode": "color", "color": "ff0000"}
    cli.main(["profile", "delete", "vermelho"])
    with pytest.raises(SystemExit, match="nome do perfil"):
        cli.main(["profile", "apply"])


def test_cli_check_pauses_and_restores_the_zone(cli_daemon, monkeypatch, capsys):
    cli, engine = cli_daemon
    cli.main(["rainbow", "d_led1"])
    monkeypatch.setattr("builtins.input", lambda prompt: "")
    cli.main(["check", "d_led1"])
    assert "d_led1: 32 LEDs -> 1 vermelho" in capsys.readouterr().out
    assert engine.calibrating == set()
    assert "d_led1" in engine.animated


def test_cli_calibrate_through_the_daemon(cli_daemon, monkeypatch):
    cli, engine = cli_daemon

    def user_looking_at_12_leds(prompt):
        lit = re.search(r"\[(\d+) LEDs\]", prompt)
        return "s" if (int(lit.group(1)) if lit else 256) >= 12 else "n"

    monkeypatch.setattr("builtins.input", user_looking_at_12_leds)
    cli.main(["calibrate", "d_led1"])
    assert engine.config.leds("d_led1") == 12
