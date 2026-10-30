<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/images/icon-white.svg">
    <img src="docs/images/icon-black.svg" alt="" width="96">
  </picture>
</p>

# aorus-rgb

Controle RGB da Gigabyte B550M Aorus Elite (ITE IT5702 / RGB Fusion 2.0) e das
memórias RGB (controlador ENE) no Linux: um daemon dono do hardware, uma CLI e
a interface gráfica **AORUS RGB**.

![A interface percorrendo tela inicial, placa e memórias](docs/images/interface.gif)

## Instalação (Arch Linux)
Baixe os pacotes da [última release](https://github.com/augustcaio/aorus-rgb/releases/latest)
e instale:

    sha256sum -c SHA256SUMS
    sudo pacman -U aorus-rgb-*.pkg.tar.zst aorus-rgb-gui-*.pkg.tar.zst
    systemctl --user enable --now aorus-rgb
    aorus-rgb calibrate d_led1   # uma vez por header ARGB, num terminal

Depois, abra **AORUS RGB** no menu. O `aorus-rgb-gui` é opcional: sem ele, a
CLI faz tudo pelo terminal.

As memórias RGB precisam também do parâmetro de kernel
`acpi_enforce_resources=lax` (veja [Desenvolvimento e uso](docs/desenvolvimento.md)).
Para outras distribuições, a release traz um wheel Python do daemon e da CLI.

**Vindo da 1.x:** o serviço passou a rodar o daemon. Depois de atualizar, como
usuário: `systemctl --user daemon-reload && systemctl --user restart aorus-rgb`.

## Branches
A `main` contém apenas documentação e licença. Todo o código vive em branches
de feature, que não compartilham arquivos com a `main`.

| Branch | Conteúdo | Estado |
|--------|----------|--------|
| `release/2.0.0` | **Versão estável 2.0.0** (tag `v2.0.0`): daemon, CLI e interface (em `gui/`); pacotes `aorus-rgb` e `aorus-rgb-gui` | estável |
| `release/1.1.0` | Versão 1.1.0 (tag `v1.1.0`): pacote do Arch e wheel Python | anterior |
| `release/1.0.0` | Versão 1.0.0 (tag `v1.0.0`): driver da placa, memórias e serviço | anterior |
| `feat/driver-it5702` | Driver USB HID, CLI, regra udev e `pyproject.toml` | incorporada na 1.0.0 |
| `feat/dram-smbus` | Memórias RGB via SMBus (ENE); ramificada de `feat/driver-it5702` | incorporada na 1.0.0 |
| `feat/service` | Perfil e serviço do systemd que aplica a iluminação no login; ramificada de `feat/dram-smbus` | incorporada na 1.0.0 |
| `feat/packaging` | `PKGBUILD`, script de build da release e serviço em `/usr/bin`; ramificada de `feat/service` | incorporada na 1.1.0 |
| `feat/daemon` | Daemon D-Bus dono do hardware, CLI cliente do daemon, perfis com nome e efeitos animados em sincronia; ramificada de `feat/packaging` | incorporada na 2.0.0 |
| `feat/gui` | Interface GTK 4 / libadwaita em Rust, ícone e `.desktop` | incorporada na 2.0.0 |

## Documentação
- [Interface gráfica](docs/interface.md)
- [Daemon D-Bus](docs/daemon.md)
- [Hardware](docs/hardware.md)
- [Protocolo IT5702](docs/protocol.md)
- [Protocolo ENE (memórias)](docs/ene.md)
- [Desenvolvimento e uso](docs/desenvolvimento.md)
- [Convenções](CONTRIBUTING.md)

Protocolo baseado no [OpenRGB](https://openrgb.org) (GPL-2.0). Projeto
independente, sem vínculo com a Gigabyte; AORUS é marca da Gigabyte.
