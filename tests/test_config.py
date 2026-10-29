from aorus_rgb.config import Config


def test_config_roundtrip(tmp_path):
    path = tmp_path / "config.json"
    Config(path).set_leds("d_led1", 9)
    assert Config(path).leds("d_led1") == 9
    assert Config(path).leds("d_led2") is None


def test_config_ignores_missing_or_corrupt_file(tmp_path):
    assert Config(tmp_path / "missing.json").leds("d_led1") is None
    bad = tmp_path / "bad.json"
    bad.write_text("{oops")
    assert Config(bad).leds("d_led1") is None


def test_default_path_follows_xdg(tmp_path, monkeypatch):
    monkeypatch.setenv("XDG_CONFIG_HOME", str(tmp_path))
    assert Config().path == tmp_path / "aorus-rgb" / "config.json"


def test_profile_roundtrip(tmp_path):
    path = tmp_path / "config.json"
    assert Config(path).profile() is None
    Config(path).set_profile(["rainbow", "all", "--speed", "3"])
    assert Config(path).profile() == ["rainbow", "all", "--speed", "3"]
