# aorus-rgb

Controle RGB da Gigabyte B550M Aorus Elite (ITE IT5702 / RGB Fusion 2.0) e das
memórias RGB (controlador ENE) no Linux.

A `main` contém apenas documentação e licença. Todo o código vive em branches
de feature, que não compartilham arquivos com a `main`.

| Branch | Conteúdo | Estado |
|--------|----------|--------|
| `feat/driver-it5702` | Driver USB HID, CLI, regra udev e `pyproject.toml` | em desenvolvimento |
| `feat/dram-smbus` | Memórias RGB via SMBus (ENE); ramificada de `feat/driver-it5702` | em desenvolvimento |
| `feat/service` | Perfil e serviço do systemd que aplica a iluminação no login; ramificada de `feat/dram-smbus` | em desenvolvimento |

## Documentação
- [Hardware](docs/hardware.md)
- [Protocolo IT5702](docs/protocol.md)
- [Protocolo ENE (memórias)](docs/ene.md)
- [Desenvolvimento e uso](docs/desenvolvimento.md)
- [Convenções](CONTRIBUTING.md)

Protocolo baseado no [OpenRGB](https://openrgb.org) (GPL-2.0).
