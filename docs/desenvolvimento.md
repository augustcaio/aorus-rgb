# Desenvolvimento

O código vive nas branches de feature (veja o [README](../README.md)). Estas
instruções valem para a branch `feat/driver-it5702`.

## Ambiente
    git switch feat/driver-it5702
    python -m venv .venv
    .venv/bin/pip install -e ".[dev]"
    .venv/bin/aorus-rgb info      # ou: python -m aorus_rgb info
    .venv/bin/pytest              # testes
    .venv/bin/ruff check .        # lint

## Permissão no hidraw (uma vez)
Sem regra udev, `/dev/hidraw*` só abre como root.

    sudo cp 70-aorus-rgb.rules /etc/udev/rules.d/
    sudo udevadm control --reload && sudo udevadm trigger

O prefixo `70-` é necessário: a tag `uaccess` precisa ser aplicada antes de
`73-seat-late.rules`.

## Uso
    aorus-rgb info
    aorus-rgb static ff0000 --header 1 --leds 32
    aorus-rgb rainbow --header 1 --leds 32
    aorus-rgb effect pulse 00ff00
    aorus-rgb off
