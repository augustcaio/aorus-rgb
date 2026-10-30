# Daemon D-Bus

A partir da 2.0, o `aorus-rgb daemon` é o único processo
que fala com o hardware. O serviço do systemd roda o daemon; a CLI e a
interface gráfica pedem a ele pelo barramento de sessão do usuário, então as
cores não disputam o hardware entre programas.

| Item | Valor |
|------|-------|
| Nome no barramento | `io.github.augustcaio.AorusRGB` (sessão) |
| Objeto | `/io/github/augustcaio/AorusRGB` |
| Interface | `io.github.augustcaio.AorusRGB1` |
| Erros | `io.github.augustcaio.AorusRGB.Error`, com a mensagem em português |

## Métodos

| Método | Entrada | Saída |
|--------|---------|-------|
| `GetZones` | | `a(ssu)`: nome, tipo (`argb`, `rgb`, `dram`), LEDs |
| `GetState` | | `a{sa{sv}}`: estado de cada zona (ver abaixo) |
| `GetDeviceInfo` | | `a{ss}`: produto, firmware, chip, caminhos, controladores das memórias |
| `SetColor` | `s` zona, `s` cor `RRGGBB` | |
| `SetEffect` | `s` zona, `s` efeito, `s` cor, `u` brilho 0-255, `u` velocidade 0-9 | |
| `SetRainbow` | `s` zona, `d` velocidade (0 = estático), `u` brilho | |
| `Off` | `s` zona | |
| `ListProfiles` | | `as` |
| `SaveProfile` | `s` nome (1-64 caracteres) | |
| `ApplyProfile` | `s` nome | |
| `DeleteProfile` | `s` nome | |
| `LightFirst` | `s` header ARGB, `u` N (1-256) | |
| `SetLedCount` | `s` header ARGB, `u` N | |
| `EndCalibration` | `s` header ARGB | |

Zonas aceitas: `d_led1`, `d_led2`, `led_c1`, `led_cpu`, `led_c2`, `dram1`,
`dram2` e os grupos `all` e `dram`. Efeitos: `static`, `pulse`, `flash`,
`cycle`, `random`. Com um grupo, `SetRainbow` ignora as zonas RGB de cor única;
com uma zona RGB isolada, devolve erro.

Sinal `StateChanged` (sem argumentos): emitido depois de qualquer mudança de
estado ou de perfis; o cliente chama `GetState` ou `ListProfiles` de novo.
Propriedade `Version` (`s`, leitura).

## Estado de uma zona

`GetState` devolve, por zona, um dicionário com `mode` e os campos do modo:

| `mode` | Campos |
|--------|--------|
| `color` | `color` (`s`, `rrggbb`) |
| `effect` | `effect` (`s`), `color` (`s`), `brightness` (`u`), `speed` (`u`) |
| `rainbow` | `speed` (`d`, 0 = estático), `brightness` (`u`) |
| `off` | |

Zonas que nunca receberam comando não aparecem.

## Calibração pelo D-Bus

`LightFirst` acende os primeiros N LEDs com uma cor distinta por LED e apaga o
resto, tirando o header da animação. O cliente pergunta ao usuário se todos os
LEDs acenderam e repete com outro N (busca binária). `SetLedCount` salva o
valor e devolve o header ao estado anterior; `EndCalibration` desiste sem
salvar. Qualquer outro comando na zona também encerra a calibração.

## Comportamento

- Na inicialização, o daemon espera até 15 s pela permissão da placa e do SMBus,
  restaura a última cena (salva em `~/.config/aorus-rgb/config.json`) ou, na
  primeira execução, liga o arco-íris nas zonas endereçáveis.
- O arco-íris animado roda num timer de 30 quadros/s que só existe enquanto há
  zona animada. A fase vem de um relógio único: zonas com a mesma velocidade
  ficam em fase mesmo se ligadas em momentos diferentes. As memórias recebem 1 de
  cada 2 quadros.
- Erro de hardware encerra o daemon com código diferente de zero, e o systemd o
  reinicia (`Restart=on-failure`). SIGINT e SIGTERM encerram com código 0.
- O arquivo de ativação (`/usr/share/dbus-1/services`) inicia o daemon pelo
  serviço do systemd quando um cliente o chama, mesmo sem estar habilitado.

Medido animando todas as zonas: ~1% de CPU e 35 MB de memória (o PyGObject e
o D-Bus explicam a diferença para os 9 MB do serviço 1.x).

## Exemplos

    gdbus introspect --session --dest io.github.augustcaio.AorusRGB \
        --object-path /io/github/augustcaio/AorusRGB
    gdbus call --session --dest io.github.augustcaio.AorusRGB \
        --object-path /io/github/augustcaio/AorusRGB \
        --method io.github.augustcaio.AorusRGB1.SetRainbow all 5.0 255

## A CLI com o daemon

Com o daemon rodando, `info`, `color`, `effect`, `rainbow`, `off`, `check`,
`calibrate` e `profile` viram chamadas D-Bus; sem ele, a CLI fala direto com o
hardware, como na 1.x. `count` e `--leds` mudam a capacidade do chip debaixo da
animação e pedem o daemon parado.

    aorus-rgb profile save noite
    aorus-rgb profile apply noite
    aorus-rgb profile            # lista

## Atualizando da 1.x

O serviço muda de `run` para o daemon. Depois de instalar o pacote 2.0:

    systemctl --user daemon-reload && systemctl --user restart aorus-rgb

Sem o `daemon-reload`, o systemd mantém a unidade antiga em memória; com o
`daemon-reload` mas sem `restart`, ele passa a esperar o nome no D-Bus de um
processo `run` que nunca o publica, e para o serviço. O perfil antigo (um
comando salvo) foi substituído por perfis com nome.
