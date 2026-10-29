# Desenvolvimento

O código vive nas branches de feature (veja o [README](../README.md)). Para
apenas usar o programa, instale o pacote da release (seção "Pacote" abaixo).
As instruções de ambiente valem para qualquer branch de código.

## Ambiente
O venv e os caches ficam fora do repositório, para que nenhuma branch
acumule arquivos gerados (`.venv`, `__pycache__`, `*.egg-info`, `.ruff_cache`).

    git switch feat/packaging
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

## Permissões (uma vez)
Sem regra udev, `/dev/hidraw*` e `/dev/i2c-*` só abrem como root. A regra dá
acesso ao usuário logado apenas ao controlador da placa e ao SMBus dos slots
de memória.

    sudo cp 70-aorus-rgb.rules /etc/udev/rules.d/
    sudo udevadm control --reload && sudo udevadm trigger

O prefixo `70-` é necessário: a tag `uaccess` precisa ser aplicada antes de
`73-seat-late.rules`.

## SMBus das memórias (uma vez)
Necessário só para as zonas `dram*` (branch `feat/dram-smbus`):

1. Adicionar `acpi_enforce_resources=lax` ao fim de `/etc/kernel/cmdline`
   (boot por UKI; com GRUB tradicional seria o `GRUB_CMDLINE_LINUX_DEFAULT`).
2. `sudo mkinitcpio -P`
3. `sudo pacman -S i2c-tools` e `echo i2c-dev | sudo tee /etc/modules-load.d/i2c-dev.conf`
4. Reiniciar e conferir: `cat /sys/class/i2c-dev/*/name` deve listar
   `SMBus PIIX4 adapter port 0 at 0b00`.

## Uso
Zonas: `d_led1`, `d_led2` (ARGB), `led_c1`, `led_cpu`, `led_c2` (RGB),
`dram1`, `dram2` (cada pente), `dram` (todos os pentes) ou `all`.

    aorus-rgb info                              # controladores, zonas e calibração
    aorus-rgb color d_led1 ff0000               # cor fixa
    aorus-rgb effect all pulse 00ff00 --speed 3 # efeito de hardware (speed 0-9)
    aorus-rgb rainbow all                       # onda de arco-íris por software (Ctrl+C)
    aorus-rgb rainbow all --speed 0             # arco-íris estático (pinta e sai)
    aorus-rgb color dram ff0000                 # memórias
    aorus-rgb off                               # apaga tudo (ou: off led_c1)

Efeitos de hardware: `static`, `pulse`, `flash`, `cycle`, `random`.

## Calibração dos headers ARGB (uma vez)
A placa não sabe quantos LEDs estão ligados em cada header. Calibre uma vez por
header, num terminal interativo, olhando as fans:

    aorus-rgb calibrate d_led1   # responde s/n; salva em ~/.config/aorus-rgb/config.json
    aorus-rgb check all          # uma cor por LED: confira que nenhum LED fica apagado

Funciona com fans em cadeia (soma dos LEDs) ou em splitter/hub (LEDs da maior
fan), de qualquer marca. Ferramentas auxiliares:

    aorus-rgb check d_led1 --leds 24   # testa outra quantidade sem salvar
    aorus-rgb count d_led1 12          # acende os primeiros 12 LEDs em branco
    aorus-rgb color d_led1 ff0000 --leds 60  # usa 60 LEDs só nesta execução

## Serviço (iluminação automática no login)
Branch `feat/service`. Escolha o perfil e instale o serviço de usuário (sem sudo):

    aorus-rgb profile rainbow all     # perfil padrão (ou: effect all pulse ff00ff, color all 00ff00...)
    aorus-rgb profile                 # mostra o perfil salvo
    ./install-service.sh              # instala, habilita e inicia
    ./install-service.sh --uninstall  # remove

O serviço roda `aorus-rgb run`, que aplica o perfil salvo (ou `rainbow all` se
não houver). Acompanhar e controlar:

    systemctl --user status aorus-rgb
    journalctl --user -u aorus-rgb -b
    systemctl --user restart aorus-rgb   # depois de trocar o perfil

No `rainbow` animado, o PC calcula a mesma fase para fans e memórias a cada
quadro, então tudo fica sincronizado. As fans recebem 30 quadros/s (USB, ~7 ms
por quadro) e as memórias 15/s, cada pente num único bloco SMBus (~7 ms para os
dois): o barramento, que o BIOS também usa, fica ocupado ~11% do tempo. Medido
no serviço: ~1% de CPU e 9 MB de memória.

No login, o serviço pode iniciar antes de o sistema dar ao usuário a permissão
da regra udev. O `run` espera até 15 s pela placa e pelo SMBus das memórias
antes de aplicar o perfil; se o tempo acabar, avisa e segue, e o reinício
automático do systemd fica como última garantia.

Suspensão: o serviço continua rodando e retoma os quadros ao acordar. Durante a
suspensão, o hub das fans DarkFlash mostra o próprio efeito (arco-íris girando)
até a placa voltar a enviar dados.

## Pacote
O pacote do Arch instala o programa em `/usr/bin/aorus-rgb`, a regra udev em
`/usr/lib/udev/rules.d`, o serviço em `/usr/lib/systemd/user` e o
`i2c-dev` em `/usr/lib/modules-load.d`. Não altera o boot.

    sudo pacman -U aorus-rgb-*.pkg.tar.zst
    systemctl --user enable --now aorus-rgb

Quem usava a instalação de desenvolvimento precisa removê-la antes, porque ela
tem prioridade: `./install-service.sh --uninstall` e
`sudo rm /etc/udev/rules.d/70-aorus-rgb.rules`. Calibração e perfil, em
`~/.config/aorus-rgb`, continuam valendo.

Gerar os artefatos de uma release, a partir do conteúdo exato da tag e fora da
pasta do projeto (requer `base-devel python-build python-installer
python-pytest`; `namcap` é opcional):

    packaging/build.sh v1.1.0      # saída em ~/.cache/aorus-rgb/dist/v1.1.0

O `namcap` acusa `aorus_rgb.__main__.main` como dependência não instalada:
é falso positivo, porque `main` é uma função e o módulo está no próprio pacote.

## Estado e configuração
| Arquivo | Conteúdo | Validade |
|---------|----------|----------|
| `~/.config/aorus-rgb/config.json` | LEDs calibrados por header e perfil | permanente |
| `~/.local/state/aorus-rgb/state.json` | máscara `0x32` e inicialização da placa | boot atual |

No primeiro comando de cada boot, a placa é inicializada (efeitos zerados e
modo beat desligado), para limpar o estado deixado por outro software.
