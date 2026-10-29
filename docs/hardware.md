# Hardware

Placa: Gigabyte **B550M Aorus Elite**. SO: Arch Linux.

## Controlador RGB
- USB `048d:5702` (ITE IT5702, "RGB Fusion 2.0"), firmware `IT5702-GIGABYTE V2.0.10.0`.
- Headers endereçáveis: D_LED1 e D_LED2 (32 LEDs padrão cada, detecção `0x07`).
- Ordem de cor reportada: `0x010002` em ambos.
- Acesso via `/dev/hidrawN` (feature reports de 64 bytes, report id `0xCC`).

## Periféricos ligados (observado)
- Memórias RGB e fan do radiador do water cooler Corsair responderam
  ao teste de cor (vermelho no D_LED1, azul no D_LED2).
- Mapeamento exato de qual header controla cada item: **pendente**.

## Pendências
- Memórias via SMBus/I2C (`i2c-dev` não carregado por padrão) caso não
  passem pelo IT5702.
- Validar ordem de cores (RGB vs GRB) visualmente.
