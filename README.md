# aorus-rgb

Controle RGB da Gigabyte B550M Aorus Elite (ITE IT5702 / RGB Fusion 2.0) e das
memórias RGB (controlador ENE) no Linux.

## Instalação (Arch Linux)
Baixe o pacote da [última release](https://github.com/augustcaio/aorus-rgb/releases/latest)
e instale:

    sha256sum -c SHA256SUMS
    sudo pacman -U aorus-rgb-*.pkg.tar.zst
    systemctl --user enable --now aorus-rgb
    aorus-rgb calibrate d_led1   # uma vez por header ARGB, num terminal

As memórias RGB precisam também do parâmetro de kernel
`acpi_enforce_resources=lax` (veja [Desenvolvimento e uso](docs/desenvolvimento.md)).
Para outras distribuições, a release traz um wheel Python.

## Branches
A `main` contém apenas documentação e licença. Todo o código vive em branches
de feature, que não compartilham arquivos com a `main`.

| Branch | Conteúdo | Estado |
|--------|----------|--------|
| `release/1.1.0` | **Versão estável 1.1.0** (tag `v1.1.0`): pacote do Arch e wheel Python | estável |
| `release/1.0.0` | Versão 1.0.0 (tag `v1.0.0`): driver da placa, memórias e serviço | anterior |
| `feat/driver-it5702` | Driver USB HID, CLI, regra udev e `pyproject.toml` | incorporada na 1.0.0 |
| `feat/dram-smbus` | Memórias RGB via SMBus (ENE); ramificada de `feat/driver-it5702` | incorporada na 1.0.0 |
| `feat/service` | Perfil e serviço do systemd que aplica a iluminação no login; ramificada de `feat/dram-smbus` | incorporada na 1.0.0 |
| `feat/packaging` | `PKGBUILD`, script de build da release e serviço em `/usr/bin`; ramificada de `feat/service` | incorporada na 1.1.0 |
| `feat/daemon` | Daemon D-Bus dono do hardware, CLI cliente do daemon e perfis com nome; ramificada de `feat/packaging` | em desenvolvimento (2.0) |

## Documentação
- [Hardware](docs/hardware.md)
- [Protocolo IT5702](docs/protocol.md)
- [Protocolo ENE (memórias)](docs/ene.md)
- [Daemon D-Bus (2.0)](docs/daemon.md)
- [Desenvolvimento e uso](docs/desenvolvimento.md)
- [Convenções](CONTRIBUTING.md)

Protocolo baseado no [OpenRGB](https://openrgb.org) (GPL-2.0).
