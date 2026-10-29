# Protocolo IT5702 (resumo)

Referência: OpenRGB, `GigabyteRGBFusion2USBController` (GPL-2.0).
Tudo por feature report de 64 bytes, byte 0 = `0xCC`.

| Cmd | Função |
|-----|--------|
| `0x60` | Lê info (get feature logo após): produto, firmware, contagem ARGB, calibração |
| `0x20 + índice` | Efeito de hardware de uma zona |
| `0x28 FF 00` | Aplica efeitos pendentes |
| `0x32 mask` | Desliga o efeito embutido por header ARGB (bit0 D_LED1, bit1 D_LED2) |
| `0x34` | Tamanho do strip por header (32/64/256/512/1024) |
| `0x58` / `0x59` | Cores diretas D_LED1 / D_LED2: offset(u16), nbytes, até 19 LEDs |

## Índices de zona (B550M AORUS ELITE)
| Zona | Índice | Registrador de efeito |
|------|--------|-----------------------|
| LED_C1 | 1 | `0x21` |
| LED_CPU | 2 | `0x22` |
| LED_C2 | 4 | `0x24` |
| D_LED1 | 5 | `0x25` |
| D_LED2 | 6 | `0x26` |

## Pacote de efeito
| Offset | Campo |
|--------|-------|
| 1 | `0x20 + índice` |
| 2 | máscara da zona, u32 (`1 << índice`) |
| 11 | tipo: 0 nenhum, 1 estático, 2 pulse, 3 flash, 4 cycle, 6 wave, 8 random |
| 12 / 13 | brilho máximo / mínimo |
| 14 | cor, u32 `0x00RRGGBB` |
| 22..29 | períodos, 4 × u16 (ms) |
| 30..33 | parâmetros (ex.: nº de cores do cycle) |

## Cores diretas x efeito embutido
Um header ARGB mostra o efeito de hardware (`0x25`/`0x26`) enquanto o bit dele
no `0x32` está desligado, e as cores diretas (`0x58`/`0x59`) quando está ligado.
O chip não informa essa máscara, então o driver guarda o último valor enviado
em `$XDG_STATE_HOME/aorus-rgb/state.json`, válido apenas para o boot atual.

## Ordem de cor
Os bytes de calibração dizem a posição de R, G e B em cada LED: offset de R em
`>> 16`, de G em `>> 8` e de B no byte baixo. `0x00010002` = GRB.
