# Hardware

Placa: Gigabyte **B550M Aorus Elite**. SO: Arch Linux.

## Controlador RGB
- USB `048d:5702` (ITE IT5702, "RGB Fusion 2.0"), firmware `IT5702-GIGABYTE V2.0.10.0`,
  chip id `0x57020100`.
- Sem modo LampArray (`support_cmd_flag = 0`), então não é preciso desligá-lo ao iniciar.
- Acesso via `/dev/hidrawN` (feature reports de 64 bytes, report id `0xCC`).

## Zonas
Layout `it5702_24` do OpenRGB, o mesmo usado pela B550M AORUS ELITE:

| Zona (CLI) | Header na placa | Tipo | LEDs |
|------------|-----------------|------|------|
| `d_led1` | D_LED1 | ARGB 5V endereçável | 32 (padrão do chip), ordem GRB |
| `d_led2` | D_LED2 | ARGB 5V endereçável | 32 (padrão do chip), ordem GRB |
| `led_c1` | LED_C1 | RGB 12V | cor única |
| `led_cpu` | LEDs da placa | RGB | cor única |
| `led_c2` | LED_C2 | RGB 12V | cor única |

## Periféricos ligados (observado)
- Memórias RGB e fan do radiador do water cooler Corsair acenderam durante
  o teste de cor (vermelho no D_LED1, azul no D_LED2).
- Qual zona controla cada item: **pendente**.

## Pendências
- Confirmar se as memórias passam pelo IT5702 ou usam SMBus/I2C
  (`i2c-dev` não carregado por padrão).
- Validar visualmente a ordem de cor (GRB) e as zonas RGB 12V e `led_cpu`.
