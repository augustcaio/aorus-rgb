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

## Periféricos
| Item | Ligação | Estado |
|------|---------|--------|
| Fans (grupo) | `d_led1` | controlados pelo driver; vermelho confirmado (ordem GRB correta) |
| Water cooler Corsair H55 RGB, fan do radiador | ARGB da placa | aceso |
| Water cooler Corsair H55 RGB, bomba | ARGB 3 pinos próprio | LEDs nunca acenderam; refrigeração funciona |
| Memórias Asgard RGB | SMBus (não passam pelo IT5702) | sempre no rainbow padrão |

Nenhum dispositivo Corsair aparece no USB (`1b1c:*`): a iluminação do H55 RGB
depende só dos cabos ARGB, e a bomba é alimentada separadamente pelo header de
fan/bomba. O cabo ARGB da bomba provavelmente está desconectado.

## SMBus (memórias)
O driver `i2c_piix4` carrega, mas não registra o barramento da AMD por conflito
com o ACPI do BIOS:

    ACPI Warning: SystemIO range 0x0B00-0x0B08 conflicts with OpRegion
    0x0B00-0x0B0F (\GSA1.SMBI)
    ACPI: OSL: Resource conflict; ACPI support missing from driver?

Só os barramentos I2C da placa de vídeo NVIDIA aparecem. Liberar o SMBus exige
o parâmetro de kernel `acpi_enforce_resources=lax`, o que permite ao Linux usar
o barramento que o BIOS também acessa.

## Pendências
- Conectar o ARGB da bomba do H55 (em `D_LED2`, nunca nos headers 12V) e testar.
- Testar `d_led2`, `led_c1`, `led_c2` e `led_cpu`.
- Memórias: liberar o SMBus, identificar o controlador RGB das Asgard e
  implementar o suporte numa branch própria.
