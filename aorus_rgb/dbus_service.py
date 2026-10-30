"""Daemon: publica o motor na sessão D-Bus do usuário.

O daemon é o único processo que fala com o hardware. A interface gráfica e a
CLI pedem a ele pelo D-Bus, então as cores não "brigam" entre programas.
"""

import importlib.metadata
import signal
import sys

import gi

gi.require_version("Gio", "2.0")
from gi.repository import Gio, GLib

from .patterns import RAINBOW_FPS

BUS_NAME = "io.github.augustcaio.AorusRGB"
OBJECT_PATH = "/io/github/augustcaio/AorusRGB"
INTERFACE = "io.github.augustcaio.AorusRGB1"
ERROR = f"{BUS_NAME}.Error"

INTROSPECTION = f"""
<node>
  <interface name="{INTERFACE}">
    <method name="GetZones"><arg name="zones" type="a(ssu)" direction="out"/></method>
    <method name="GetState"><arg name="state" type="a{{sa{{sv}}}}" direction="out"/></method>
    <method name="GetDeviceInfo"><arg name="info" type="a{{ss}}" direction="out"/></method>
    <method name="SetColor">
      <arg name="zone" type="s" direction="in"/><arg name="color" type="s" direction="in"/>
    </method>
    <method name="SetEffect">
      <arg name="zone" type="s" direction="in"/><arg name="effect" type="s" direction="in"/>
      <arg name="color" type="s" direction="in"/><arg name="brightness" type="u" direction="in"/>
      <arg name="speed" type="u" direction="in"/>
    </method>
    <method name="SetRainbow">
      <arg name="zone" type="s" direction="in"/><arg name="speed" type="d" direction="in"/>
      <arg name="brightness" type="u" direction="in"/>
    </method>
    <method name="Off"><arg name="zone" type="s" direction="in"/></method>
    <method name="ListProfiles"><arg name="names" type="as" direction="out"/></method>
    <method name="SaveProfile"><arg name="name" type="s" direction="in"/></method>
    <method name="ApplyProfile"><arg name="name" type="s" direction="in"/></method>
    <method name="DeleteProfile"><arg name="name" type="s" direction="in"/></method>
    <method name="LightFirst">
      <arg name="zone" type="s" direction="in"/><arg name="count" type="u" direction="in"/>
    </method>
    <method name="SetLedCount">
      <arg name="zone" type="s" direction="in"/><arg name="count" type="u" direction="in"/>
    </method>
    <method name="EndCalibration"><arg name="zone" type="s" direction="in"/></method>
    <signal name="StateChanged"/>
    <property name="Version" type="s" access="read"/>
  </interface>
</node>
"""


def version():
    try:
        return importlib.metadata.version("aorus-rgb")
    except importlib.metadata.PackageNotFoundError:
        return "dev"


def log(msg):
    print(f"aorus-rgb: {msg}", file=sys.stderr, flush=True)


def _variant(value):
    if isinstance(value, str):
        return GLib.Variant("s", value)
    if isinstance(value, float):
        return GLib.Variant("d", value)
    return GLib.Variant("u", int(value))


class Service:
    """Liga o motor a uma conexão D-Bus e mantém o timer da animação."""

    def __init__(self, engine, on_fatal=None):
        self.engine = engine
        self.on_fatal = on_fatal
        self.connection = None
        self.timer = None
        self.context = None
        engine.listeners.append(self.emit_state_changed)
        self.methods = {
            "GetZones": lambda: GLib.Variant("(a(ssu))", (engine.zone_info(),)),
            "GetState": self.get_state,
            "GetDeviceInfo": lambda: GLib.Variant("(a{ss})", (engine.device_info(),)),
            "SetColor": engine.set_color,
            "SetEffect": engine.set_effect,
            "SetRainbow": engine.set_rainbow,
            "Off": engine.off,
            "ListProfiles": lambda: GLib.Variant("(as)", (engine.list_profiles(),)),
            "SaveProfile": engine.save_profile,
            "ApplyProfile": engine.apply_profile,
            "DeleteProfile": engine.delete_profile,
            "LightFirst": engine.light_first,
            "SetLedCount": engine.set_led_count,
            "EndCalibration": engine.end_calibration,
        }

    def register(self, connection):
        """Publica o objeto; o contexto atual da thread recebe chamadas e o timer."""
        self.connection = connection
        self.context = GLib.MainContext.ref_thread_default()
        node = Gio.DBusNodeInfo.new_for_xml(INTROSPECTION)
        return connection.register_object(OBJECT_PATH, node.interfaces[0], self._on_call, self._on_get_property)

    def get_state(self):
        state = {name: {k: _variant(v) for k, v in spec.items()} for name, spec in self.engine.scene.items()}
        return GLib.Variant("(a{sa{sv}})", (state,))

    def emit_state_changed(self):
        if self.connection is not None:
            self.connection.emit_signal(None, OBJECT_PATH, INTERFACE, "StateChanged", None)

    # --- chamadas ----------------------------------------------------------------
    def _on_call(self, connection, sender, path, interface, method, parameters, invocation):
        try:
            result = self.methods[method](*parameters.unpack())
        except (ValueError, KeyError) as e:
            invocation.return_dbus_error(ERROR, str(e))
            return
        except OSError as e:
            invocation.return_dbus_error(ERROR, f"falha no hardware: {e}")
            self._fatal(e)
            return
        invocation.return_value(result if isinstance(result, GLib.Variant) else None)
        self.update_timer()

    def _on_get_property(self, connection, sender, path, interface, name):
        return GLib.Variant("s", version()) if name == "Version" else None

    # --- animação ----------------------------------------------------------------
    def update_timer(self):
        """Liga o timer quando há zona animada e o desliga quando não há."""
        if self.engine.animated and self.timer is None:
            self.timer = GLib.timeout_source_new(1000 // RAINBOW_FPS)
            self.timer.set_callback(self._tick)
            self.timer.attach(self.context)
        elif not self.engine.animated and self.timer is not None:
            self.timer.destroy()
            self.timer = None

    def _tick(self, *args):
        try:
            self.engine.tick()
        except OSError as e:
            self._fatal(e)
            return GLib.SOURCE_REMOVE
        if self.engine.animated:
            return GLib.SOURCE_CONTINUE
        self.timer = None
        return GLib.SOURCE_REMOVE

    def _fatal(self, error):
        log(f"erro de hardware, encerrando para o systemd reiniciar: {error}")
        if self.on_fatal:
            self.on_fatal(error)


def run(engine):
    """Laço principal do daemon; devolve o código de saída."""
    loop = GLib.MainLoop()
    exit_code = 0

    def fatal(error):
        nonlocal exit_code
        exit_code = 1
        loop.quit()

    def name_lost(connection, name):
        nonlocal exit_code
        if connection is None:
            log("sessão D-Bus indisponível")
        else:
            log(f"{name} já está em uso: outro daemon está rodando")
        exit_code = 1
        loop.quit()

    def name_acquired(connection, name):
        log(f"pronto em {name}")
        service.update_timer()

    def stop(*_):
        loop.quit()
        return GLib.SOURCE_REMOVE

    service = Service(engine, on_fatal=fatal)
    owner = Gio.bus_own_name(
        Gio.BusType.SESSION, BUS_NAME, Gio.BusNameOwnerFlags.NONE,
        lambda connection, name: service.register(connection),
        name_acquired,
        name_lost,
    )
    for sig in (signal.SIGINT, signal.SIGTERM):
        GLib.unix_signal_add(GLib.PRIORITY_DEFAULT, sig, stop)
    loop.run()
    Gio.bus_unown_name(owner)
    return exit_code
