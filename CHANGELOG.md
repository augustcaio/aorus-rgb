# Changelog

Formato baseado em [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/),
versionamento [SemVer](https://semver.org/lang/pt-BR/). Commits seguem
[Conventional Commits](https://www.conventionalcommits.org/pt-br/).

## [Unreleased]

### Added
- `feat/driver-it5702`: driver USB HID para o controlador ITE IT5702
  (RGB Fusion 2.0), regra udev e `pyproject.toml` com extras de desenvolvimento.
- Controle por zona: `d_led1`, `d_led2`, `led_c1`, `led_cpu`, `led_c2` e `all`.
- CLI: `info`, `color`, `effect`, `rainbow`, `off`.
- Efeitos de hardware `wave` e `random`, velocidade 0-9 e brilho 0-255 validados.
- Testes unitários (50) com fixture capturada do hardware real.
- `feat/dram-smbus`: memórias RGB com controlador ENE `AUDA0-E6K5-0101` via
  SMBus, como zonas `dram1`, `dram2` e `dram`; remapeamento automático dos
  pentes que dividem o endereço `0x77`.
- Acesso ao SMBus sem dependências, com bloqueio dos endereços da SPD.
- Regra udev para o SMBus dos slots de memória.

### Changed
- **Quebra:** `static COR --header N` virou `color d_ledN COR`, e `effect`
  recebe a zona como primeiro argumento.

### Fixed
- Definir cor em um header ARGB religava o efeito embutido do outro.
