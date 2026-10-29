# aorus-rgb

Controle RGB da Gigabyte B550M Aorus Elite (ITE IT5702 / RGB Fusion 2.0) no Linux.

A `main` contém apenas documentação e licença. Todo o código vive em branches
de feature, que não compartilham arquivos com a `main`.

| Branch | Conteúdo | Estado |
|--------|----------|--------|
| `feat/driver-it5702` | Driver USB HID, CLI, regra udev e `pyproject.toml` | em desenvolvimento |

## Documentação
- [Hardware](docs/hardware.md)
- [Protocolo IT5702](docs/protocol.md)
- [Desenvolvimento e uso](docs/desenvolvimento.md)
- [Convenções](CONTRIBUTING.md)

Protocolo baseado no [OpenRGB](https://openrgb.org) (GPL-2.0).
