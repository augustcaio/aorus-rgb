# aorus-rgb

Controle RGB da Gigabyte B550M Aorus Elite (ITE IT5702 / RGB Fusion 2.0) e das
memórias RGB (controlador ENE) no Linux.

A `main` contém apenas documentação e licença. Todo o código vive em branches
de feature, que não compartilham arquivos com a `main`.

| Branch | Conteúdo | Estado |
|--------|----------|--------|
| `release/1.0.0` | **Versão estável 1.0.0** (tag `v1.0.0`): driver da placa, memórias e serviço | estável |
| `feat/driver-it5702` | Driver USB HID, CLI, regra udev e `pyproject.toml` | incorporada na 1.0.0 |
| `feat/dram-smbus` | Memórias RGB via SMBus (ENE); ramificada de `feat/driver-it5702` | incorporada na 1.0.0 |
| `feat/service` | Perfil e serviço do systemd que aplica a iluminação no login; ramificada de `feat/dram-smbus` | incorporada na 1.0.0; base para o desenvolvimento seguinte |

## Documentação
- [Hardware](docs/hardware.md)
- [Protocolo IT5702](docs/protocol.md)
- [Protocolo ENE (memórias)](docs/ene.md)
- [Desenvolvimento e uso](docs/desenvolvimento.md)
- [Convenções](CONTRIBUTING.md)

Protocolo baseado no [OpenRGB](https://openrgb.org) (GPL-2.0).
