"""Cliente do daemon para a CLI.

Com o daemon rodando, a CLI pede tudo a ele: só um processo fala com o
hardware e as cores não "brigam" com a animação. Sem o daemon (ou sem
PyGObject), a CLI fala direto com o hardware, como na 1.x.
"""


def connect():
    """Cliente do daemon, ou None se ele não estiver rodando."""
    try:
        from gi.repository import Gio, GLib

        from .dbus_service import BUS_NAME
    except (ImportError, ValueError):
        return None
    try:
        bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
        (running,) = bus.call_sync(
            "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus", "NameHasOwner",
            GLib.Variant("(s)", (BUS_NAME,)), GLib.VariantType("(b)"), Gio.DBusCallFlags.NONE, 2000, None,
        ).unpack()
    except GLib.Error:
        return None
    return DaemonClient(bus) if running else None


class DaemonClient:
    def __init__(self, connection):
        self.connection = connection

    def call(self, method, signature=None, *args):
        from gi.repository import Gio, GLib

        from .dbus_service import BUS_NAME, INTERFACE, OBJECT_PATH

        params = GLib.Variant(signature, args) if signature else None
        try:
            reply = self.connection.call_sync(
                BUS_NAME, OBJECT_PATH, INTERFACE, method, params, None, Gio.DBusCallFlags.NONE, 10_000, None
            )
        except GLib.Error as e:
            if Gio.DBusError.is_remote_error(e):
                # o PyGObject não remove o prefixo "GDBus.Error:<nome>: " da mensagem
                prefix = f"GDBus.Error:{Gio.DBusError.get_remote_error(e)}: "
                raise ValueError(e.message.removeprefix(prefix)) from None
            raise ValueError(f"daemon não respondeu: {e.message}") from None
        return reply.unpack() if reply is not None else ()

    def zones(self):
        return {name: (kind, leds) for name, kind, leds in self.call("GetZones")[0]}
