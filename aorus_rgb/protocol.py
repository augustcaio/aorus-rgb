"""Protocolo do ITE IT5702 (RGB Fusion 2.0): montagem e leitura de pacotes.

Funções puras, sem acesso a hardware. Baseado no driver do OpenRGB
(GigabyteRGBFusion2USBController, GPL-2.0). Todo pacote é um feature
report de 64 bytes com report id 0xCC.
"""

import struct
from dataclasses import dataclass

REPORT_ID = 0xCC
PACKET_SIZE = 64

CMD_EFFECT_BASE = 0x20       # 0x20 + índice da zona
CMD_APPLY = 0x28
CMD_BUILTIN_EFFECT_OFF = 0x32
CMD_LED_COUNT = 0x34
CMD_INFO = 0x60

ARGB_LEDS_PER_PACKET = 19
LED_COUNT_STEPS = (32, 64, 256, 512, 1024)  # índice = código enviado ao chip
SPEED_MAX = 9
# O pulse apaga os LEDs com brilho máximo acima de 100 (confirmado na
# B550M AORUS ELITE; o OpenRGB aplica o mesmo limite). A escala 0-255 da CLI
# é comprimida para 0-100 nesse efeito.
PULSE_BRIGHTNESS_MAX = 100

EFFECTS = {
    "none": 0,
    "static": 1,
    "pulse": 2,
    "flash": 3,
    "cycle": 4,
    "wave": 6,
    "random": 8,
}


@dataclass(frozen=True)
class Zone:
    name: str
    index: int                  # posição no registrador de efeitos
    argb_reg: int | None = None  # registrador de cores diretas (só ARGB)
    builtin_bit: int = 0        # bit no comando 0x32 (só ARGB)
    cal_slot: int = 0           # qual calibração de ordem de cor usar (só ARGB)

    @property
    def addressable(self):
        return self.argb_reg is not None


# Layout "it5702_24" do OpenRGB, usado pela B550M AORUS ELITE
ZONES = {
    z.name: z
    for z in (
        Zone("d_led1", 5, argb_reg=0x58, builtin_bit=0x01, cal_slot=0),
        Zone("d_led2", 6, argb_reg=0x59, builtin_bit=0x02, cal_slot=1),
        Zone("led_c1", 1),
        Zone("led_cpu", 2),
        Zone("led_c2", 4),
    )
}

@dataclass(frozen=True)
class DeviceInfo:
    product: str
    firmware: str
    device_num: int
    strip_detect: int
    support_flags: int
    argb_counts: tuple
    calibration: tuple  # cal_strip0, cal_strip1
    chip_id: int


def command(*data):
    """Pacote simples: report id + bytes de dados, completado com zeros."""
    buf = bytearray(PACKET_SIZE)
    buf[0] = REPORT_ID
    buf[1:1 + len(data)] = bytes(data)
    return bytes(buf)


def parse_info(raw):
    """Decodifica a resposta do comando 0x60 (struct IT8297Report)."""
    if len(raw) < PACKET_SIZE or raw[0] != REPORT_ID:
        raise ValueError(f"resposta de info inválida: {bytes(raw[:4]).hex()}")
    fw = struct.unpack_from("<I", raw, 4)[0]
    nibbles = [n for b in raw[8:11] for n in (b & 0x0F, b >> 4)]
    _, cal0, cal1, _, chip_id, _ = struct.unpack_from("<6I", raw, 40)
    return DeviceInfo(
        product=raw[12:40].split(b"\0")[0].decode("ascii", "replace"),
        firmware=".".join(str((fw >> s) & 0xFF) for s in (0, 8, 16, 24)),
        device_num=raw[2],
        strip_detect=raw[3],
        support_flags=raw[11],
        argb_counts=tuple(LED_COUNT_STEPS[min(n, len(LED_COUNT_STEPS) - 1)] for n in nibbles),
        calibration=(cal0, cal1),
        chip_id=chip_id,
    )


def byte_order(cal):
    """Offsets de (R, G, B) dentro de cada LED, a partir da calibração."""
    return (cal >> 16) & 0xFF, (cal >> 8) & 0xFF, cal & 0xFF


def color_order_name(cal):
    """Ordem dos bytes no fio, ex.: offsets (1, 0, 2) -> "GRB"."""
    offsets = byte_order(cal)
    if sorted(offsets) != [0, 1, 2]:
        return f"{cal:#08x}"
    name = [""] * 3
    for channel, pos in zip("RGB", offsets):
        name[pos] = channel
    return "".join(name)


def led_count_code(count):
    """Menor degrau de tamanho de strip que comporta `count` LEDs."""
    return next((i for i, n in enumerate(LED_COUNT_STEPS) if count <= n), len(LED_COUNT_STEPS) - 1)


def led_count_packet(counts):
    """Tamanho dos 6 strips ARGB (o IT5702 usa só os 2 primeiros)."""
    codes = [led_count_code(c) for c in counts]
    return command(CMD_LED_COUNT, codes[1] << 4 | codes[0], codes[3] << 4 | codes[2], codes[5] << 4 | codes[4])


def builtin_effect_off_packet(mask):
    """Bits ligados = header ARGB exibe cores diretas em vez do efeito embutido."""
    return command(CMD_BUILTIN_EFFECT_OFF, mask)


def apply_packet():
    return command(CMD_APPLY, 0xFF, 0x00)


def _timings(effect, speed):
    """(periods[4], params[4]) no padrão do software da Gigabyte."""
    p, a = [0, 0, 0, 0], [0, 0, 0, 0]
    if effect == "pulse":
        p[0] = p[1] = 400 + speed * 100 if speed <= 6 else 1000 + (speed - 6) * 200
        p[2] = 200
    elif effect == "flash":
        p[0], p[1], p[2] = 100, 100, speed * 200 + 700
    elif effect == "cycle":
        p[0] = speed * 100 + 300 + (1300 * (speed - 8) if speed > 8 else 0)
        p[1] = p[0] - 200
        a[0] = 7
    elif effect == "wave":
        p[0] = ((speed + 1) ** 2 + (speed + 1) + 10) * 5 // 2
        a[0], a[1] = 7, 1
    elif effect == "random":
        p[0] = 100
        a[0], a[1] = 1, 5
    return p, a


def effect_packet(zone, effect, color, brightness=255, speed=5):
    """Efeito de hardware numa zona (struct PktEffect)."""
    if effect not in EFFECTS:
        raise ValueError(f"efeito desconhecido: {effect}")
    if not 0 <= speed <= SPEED_MAX:
        raise ValueError(f"velocidade deve estar entre 0 e {SPEED_MAX}")
    if not 0 <= brightness <= 255:
        raise ValueError("brilho deve estar entre 0 e 255")
    r, g, b = color
    periods, params = _timings(effect, speed)
    buf = bytearray(PACKET_SIZE)
    buf[0] = REPORT_ID
    buf[1] = CMD_EFFECT_BASE + zone.index
    struct.pack_into("<II", buf, 2, 1 << zone.index, 0)
    if effect == "pulse":
        brightness = brightness * PULSE_BRIGHTNESS_MAX // 255
    buf[11], buf[12], buf[13] = EFFECTS[effect], brightness, 0
    struct.pack_into("<II", buf, 14, r << 16 | g << 8 | b, 0)
    struct.pack_into("<4H", buf, 22, *periods)
    buf[30:34] = bytes(params)
    return bytes(buf)


def argb_packets(zone, colors, cal):
    """Cores diretas para um header ARGB, até 19 LEDs por pacote."""
    if not zone.addressable:
        raise ValueError(f"{zone.name} não é endereçável")
    bo_r, bo_g, bo_b = byte_order(cal)
    packets = []
    offset = 0
    for i in range(0, len(colors), ARGB_LEDS_PER_PACKET):
        chunk = colors[i:i + ARGB_LEDS_PER_PACKET]
        buf = bytearray(PACKET_SIZE)
        buf[0], buf[1] = REPORT_ID, zone.argb_reg
        struct.pack_into("<H", buf, 2, offset)
        buf[4] = len(chunk) * 3
        for j, (r, g, b) in enumerate(chunk):
            base = 5 + j * 3
            buf[base + bo_r], buf[base + bo_g], buf[base + bo_b] = r, g, b
        packets.append(bytes(buf))
        offset += len(chunk) * 3
    return packets
