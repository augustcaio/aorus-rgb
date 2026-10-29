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
| Memórias Asgard RGB (2 pentes) | SMBus, controlador ENE `AUDA0-E6K5-0101` em `0x77` | sempre no rainbow padrão; suporte em `feat/dram-smbus` |

Nenhum dispositivo Corsair aparece no USB (`1b1c:*`): a iluminação do H55 RGB
depende só dos cabos ARGB, e a bomba é alimentada separadamente pelo header de
fan/bomba. O cabo ARGB da bomba provavelmente está desconectado.

## SMBus (memórias)
Por padrão o driver `i2c_piix4` carrega, mas não registra o barramento da AMD,
por conflito com o ACPI do BIOS:

    ACPI Warning: SystemIO range 0x0B00-0x0B08 conflicts with OpRegion
    0x0B00-0x0B0F (\GSA1.SMBI)
    ACPI: OSL: Resource conflict; ACPI support missing from driver?

Resolvido com o parâmetro de kernel `acpi_enforce_resources=lax` (no
`/etc/kernel/cmdline`, já que o boot usa UKI) e o módulo `i2c-dev`. O aviso
continua no log, mas os barramentos passam a existir:

| Nó | Adaptador | Uso |
|----|-----------|-----|
| `i2c-0` | SMBus PIIX4 adapter port 0 at 0b00 | slots de memória |
| `i2c-1` | SMBus PIIX4 adapter port 2 at 0b00 | auxiliar |
| `i2c-2` | SMBus PIIX4 adapter port 1 at 0b20 | auxiliar |

Varredura só de leitura (`i2cdetect -r`) do `i2c-0`, sem tocar em `0x30-0x37`
e `0x50-0x57` (SPD):

| Endereço | Dispositivo |
|----------|-------------|
| `0x0c`, `0x28`, `0x60`, `0x68` | dispositivos da placa |
| `0x77` | controlador RGB ENE das memórias (os 2 pentes, ainda não remapeados) |

O registrador `0x1000` do `0x77` devolve `AUDA0-E6K5-0101`: controlador ENE de
DRAM de segunda geração, suportado pelo OpenRGB. Detalhes em
[Protocolo ENE](ene.md).

## Pendências
- Conectar o ARGB da bomba do H55 (em `D_LED2`, nunca nos headers 12V) e testar.
- Testar `d_led2`, `led_c1`, `led_c2` e `led_cpu`.
- Memórias: testar no hardware o remapeamento e as cores (`feat/dram-smbus`).
