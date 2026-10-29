# Protocolo IT5702 (resumo)

Referência: OpenRGB, `GigabyteRGBFusion2USBController` (GPL-2.0).
Tudo por feature report de 64 bytes, byte 0 = `0xCC`.

| Cmd | Função |
|-----|--------|
| `0x60` | Lê info (get feature logo após): produto, firmware, contagem ARGB, calibração |
| `0x20 + índice` | Efeito de hardware de uma zona |
| `0x28` + máscara u32 | Aplica os efeitos das zonas na máscara (`0xFF` = todas) |
| `0x31 0/1` | Modo "beat" (efeitos acompanhando áudio) |
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

## Armadilhas encontradas na B550M AORUS ELITE

**Estado deixado por outro software.** Depois de usar o RGB Fusion no Windows,
os efeitos animados (`pulse`, `cycle`, `random`) deixavam os headers ARGB
apagados, enquanto `static` e `flash` funcionavam. A inicialização do OpenRGB
resolve: zerar `0x20`-`0x27`, aplicar `0xFF` e desligar o beat (`0x31 00`). O
driver faz isso uma vez por boot, e não a cada comando, porque zerar os efeitos
desfaria as cores das outras zonas.

**Aplicar só o que mudou.** O `0x28` reaplica todo registrador cuja zona está
na máscara, mesmo sem mudança. Aplicar sempre `0xFF` reaplicava sobras, como
um efeito "todas as zonas" no `0x20`, por cima do efeito novo. O driver aplica
só as zonas alteradas, como o OpenRGB.

**Pulse com brilho acima de 100.** Apaga os LEDs em vez de pulsar (o OpenRGB
aplica o mesmo limite). A escala 0-255 da CLI é comprimida para 0-100 no pulse.

**Wave de hardware.** O firmware só o executa na placa inteira: todas as zonas
em estático num pacote `0x20` com máscara `0xFF` e o wave no slot 2 (`0x22`).
Nesta placa, esse estado mantém os headers ARGB apagados até um reset dos
registradores. O efeito foi removido; a onda de arco-íris é feita por software
(`rainbow`).

**Quantidade de LEDs.** O `0x34` só guarda degraus (32, 64, 256...) e o chip
não detecta quantos LEDs estão ligados. O número real vem da calibração
(`calibrate`) e fica em `$XDG_CONFIG_HOME/aorus-rgb/config.json`.

## Ordem de cor
Os bytes de calibração dizem a posição de R, G e B em cada LED: offset de R em
`>> 16`, de G em `>> 8` e de B no byte baixo. `0x00010002` = GRB.
