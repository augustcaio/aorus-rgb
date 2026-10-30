"""Testes do daemon pelo D-Bus, num barramento privado (Gio.TestDBus)."""

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
