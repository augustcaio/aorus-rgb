# Desenvolvimento

O código vive nas branches de feature (veja o [README](../README.md)). Estas
instruções valem para a branch `feat/driver-it5702`.

## Ambiente
O venv e os caches ficam fora do repositório, para que nenhuma branch
acumule arquivos gerados (`.venv`, `__pycache__`, `*.egg-info`, `.ruff_cache`).

    git switch feat/driver-it5702
    ./dev-setup.sh
    source ~/.venvs/aorus-rgb/bin/activate
    aorus-rgb info                # ou: python -m aorus_rgb info
    pytest                        # testes
    ruff check .                  # lint

| O quê | Onde |
|-------|------|
| venv | `~/.venvs/aorus-rgb` |
| bytecode (`__pycache__`) | `~/.cache/aorus-rgb/pycache` |
| cache do ruff | `~/.cache/aorus-rgb/ruff` |

O pacote não é instalado no venv: um arquivo `.pth` aponta para a pasta do
repositório, então não é gerado `*.egg-info`. Use sempre o Python do venv; o
Python do sistema ainda criaria `__pycache__` na pasta.

### Ao trocar de branch
O git não remove arquivos ignorados ao trocar de branch. Se algo gerado
aparecer, limpe antes de trocar (apaga só arquivos ignorados, nunca código
versionado):

    git clean -fdX

## Permissão no hidraw (uma vez)
Sem regra udev, `/dev/hidraw*` só abre como root.

    sudo cp 70-aorus-rgb.rules /etc/udev/rules.d/
    sudo udevadm control --reload && sudo udevadm trigger

O prefixo `70-` é necessário: a tag `uaccess` precisa ser aplicada antes de
`73-seat-late.rules`.

## Uso
Zonas: `d_led1`, `d_led2` (ARGB), `led_c1`, `led_cpu`, `led_c2` (RGB) ou `all`.

    aorus-rgb info                              # controlador e zonas
    aorus-rgb color d_led1 ff0000               # cor fixa
    aorus-rgb color d_led1 ff0000 --leds 60     # ajusta o tamanho do strip
    aorus-rgb effect all pulse 00ff00 --speed 3 # efeito de hardware (speed 0-9)
    aorus-rgb rainbow d_led2                    # animação por software (Ctrl+C)
    aorus-rgb off                               # apaga tudo (ou: off led_c1)

Efeitos: `static`, `pulse`, `flash`, `cycle`, `wave`, `random`.
