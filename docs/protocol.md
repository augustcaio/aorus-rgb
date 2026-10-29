# Protocolo IT5702 (resumo)

Referência: OpenRGB, `GigabyteRGBFusion2USBController` (GPL-2.0).
Tudo por feature report de 64 bytes, byte 0 = `0xCC`.

| Cmd | Função |
|-----|--------|
| `0x60` | Lê info (get feature logo após): produto, firmware, contagem ARGB, calibração |
| `0x20-0x27` | Efeito de hardware por zona LED1..8 |
| `0x28 FF 00` | Aplica efeitos pendentes |
| `0x32 mask` | Desliga efeito embutido por header (bit0 D_LED1, bit1 D_LED2) |
| `0x34` | Tamanho do strip por header (32/64/256/512/1024) |
| `0x58` / `0x59` | Cores diretas D_LED1 / D_LED2: offset(u16), nbytes, até 19 LEDs |

Layout do efeito: zona u32 @2, tipo @11, brilho @12, cor BGR u32 @14, períodos u16 @22..28, params @30.
