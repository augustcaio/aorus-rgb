import pytest

from aorus_rgb.device import Controller, MemoryState

# Resposta real do comando 0x60 na B550M AORUS ELITE (firmware V2.0.10.0)
REAL_INFO = bytes.fromhex(
    "cc 01 00 07 02 00 0a 00 00 00 00 00 49 54 35 37 30 32 2d 47 49 47 41 42 59 54 45 20 56 32 2e 30"
    "2e 31 30 2e 30 00 00 00 00 01 02 00 02 00 01 00 02 00 01 00 00 01 02 00 00 01 02 57 00 00 00 00"
)


class FakeTransport:
    """Registra os pacotes enviados e devolve uma resposta fixa ao get()."""

    path = "/dev/fake"

    def __init__(self, info=REAL_INFO):
        self.info = info
        self.sent = []

    def send(self, packet):
        assert len(packet) == 64 and packet[0] == 0xCC
        self.sent.append(bytes(packet))

    def get(self):
        return self.info

    def close(self):
        pass

    def commands(self):
        return [pkt[1] for pkt in self.sent]


@pytest.fixture(autouse=True)
def isolated_user_dirs(tmp_path, monkeypatch):
    """Nenhum teste lê ou grava ~/.config ou ~/.local/state de verdade."""
    monkeypatch.setenv("XDG_CONFIG_HOME", str(tmp_path / "config"))
    monkeypatch.setenv("XDG_STATE_HOME", str(tmp_path / "state"))


@pytest.fixture
def transport():
    return FakeTransport()


@pytest.fixture
def controller(transport, monkeypatch):
    monkeypatch.setattr("aorus_rgb.device.time.sleep", lambda s: None)
    c = Controller(transport, MemoryState())
    transport.sent.clear()
    return c
