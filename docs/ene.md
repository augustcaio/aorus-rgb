# Protocolo ENE (memórias RGB)

Referência: OpenRGB, `ENESMBusController` (GPL-2.0). Controlador encontrado nas
memórias Asgard: **`AUDA0-E6K5-0101`** (ENE DRAM, segunda geração).

## Acesso a registradores
Os registradores têm 16 bits e são acessados por um ponteiro:

| Operação | SMBus |
|----------|-------|
| Selecionar registrador | write word, comando `0x00`, valor com os bytes invertidos (`0x1000` → `0x0010`) |
| Ler | read byte, comando `0x81` |
| Escrever | write byte, comando `0x01` |
| Escrever bloco | write block, comando `0x03` (até 3 bytes por vez no PIIX4) |

## Registradores
| Registrador | Função |
|-------------|--------|
| `0x1000` | Nome/versão do chip, 16 bytes |
| `0x1C00` | Tabela de configuração, 64 bytes (byte `0x02` = nº de LEDs) |
| `0x8020` | Modo direto: 1 = cores do PC, 0 = efeito do chip |
| `0x8021` | Modo do efeito |
| `0x8022` | Velocidade: 0 (mais rápido) a 4 (mais lento) |
| `0x8023` | Direção: 0 normal, 1 inversa |
| `0x80A0` | Aplicar: `0x01`. `0xAA` grava na flash do chip (não usado) |
| `0x80F8` | Índice do slot (só memórias) |
| `0x80F9` | Novo endereço I2C, deslocado 1 bit à esquerda (só memórias) |
| `0x8100` | Cores diretas (AUDA0), 3 bytes por LED na ordem **R, B, G** |
| `0x8160` | Cores dos efeitos (AUDA0), mesma ordem |

## Modos
0 desligado, 1 estático, 2 respiração, 3 piscar, 4 ciclo de cores, 5 rainbow,
6 ciclo com respiração, 7 chase com fade, 8 ciclo + chase com fade, 9 chase,
10 ciclo + chase, 11 onda de cores, 12 chase rainbow pulse, 13 random flicker.

## Detecção e remapeamento
1. **Assinatura (só leitura):** os registradores SMBus `0xA0`-`0xAF` devolvem
   `0, 1, ..., 15`.
2. Todo pente liga em `0x77`. Com dois pentes, os dois respondem juntos: as
   leituras funcionam (dados iguais), mas qualquer escrita vai para ambos.
3. Para cada slot de 0 a 7, enquanto houver alguém em `0x77`: escreve o slot em
   `0x80F8` e o próximo endereço livre de `0x70`-`0x76` (deslocado) em `0x80F9`.
   O pente daquele slot passa a responder no endereço novo.
4. O remapeamento vale até o PC ser desligado; a CLI refaz a cada execução, se
   preciso.

## Segurança
O mesmo barramento tem a EEPROM SPD (`0x50`-`0x57`), com os timings que o BIOS
usa no boot. A camada SMBus do projeto recusa qualquer acesso a `0x50`-`0x57`
e a `0x30`-`0x37` (comandos de proteção/página da SPD DDR4).
