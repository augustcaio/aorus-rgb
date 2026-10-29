"""Configuração do usuário, persistente entre boots (ex.: LEDs calibrados)."""

import json
import os
from pathlib import Path


class Config:
    def __init__(self, path=None):
        if path is None:
            base = os.environ.get("XDG_CONFIG_HOME") or Path.home() / ".config"
            path = Path(base) / "aorus-rgb" / "config.json"
        self.path = Path(path)
        try:
            self.data = json.loads(self.path.read_text())
        except (OSError, ValueError):
            self.data = {}

    def leds(self, zone_name):
        """Quantidade de LEDs calibrada para a zona, ou None."""
        value = self.data.get("zones", {}).get(zone_name, {}).get("leds")
        return int(value) if value else None

    def set_leds(self, zone_name, count):
        self.data.setdefault("zones", {}).setdefault(zone_name, {})["leds"] = count
        self.save()

    def save(self):
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self.path.write_text(json.dumps(self.data, indent=2) + "\n")


class MemoryConfig(Config):
    """Configuração que não toca o disco (testes)."""

    def __init__(self):
        self.data = {}

    def save(self):
        pass
