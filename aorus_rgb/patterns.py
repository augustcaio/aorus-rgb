"""Padrões de cor usados pela CLI e pelo daemon: arco-íris, efeitos animados e
conferência de LEDs."""

import colorsys
import math


def rainbow_frame(n, t, value):
    """Arco-íris de n LEDs com fase t (0-1)."""
    return [tuple(int(x * 255) for x in colorsys.hsv_to_rgb((t + i / n) % 1, 1, value)) for i in range(n)]


RAINBOW_FPS = 30
# As memórias recebem 1 de cada N quadros, sempre com a mesma fase: continuam
# sincronizadas com as fans e o SMBus (que o BIOS também usa) fica ocupado
# ~10% do tempo em vez de ~21%.
DRAM_FRAME_DIVIDER = 2


# Efeitos desenhados pelo daemon, quadro a quadro, num relógio comum a todas as
# zonas: assim a placa e as memórias pulsam e trocam de cor juntas. Os modos de
# hardware de cada chip têm períodos e relógios próprios e se desencontram. Os
# tempos seguem os que o software da Gigabyte manda para o IT5702
# (velocidade 0 = rápido .. 9 = lento).
ANIMATED_EFFECTS = ("pulse", "flash", "cycle", "random")
CYCLE_COLORS = 7


def pulse_ramp(speed):
    """Segundos para acender (e para apagar) no pulso."""
    return (400 + speed * 100 if speed <= 6 else 1000 + (speed - 6) * 200) / 1000


def flash_period(speed):
    return 0.2 + (speed * 200 + 700) / 1000


def cycle_step(speed):
    """Segundos por cor no ciclo de 7 cores."""
    return (speed * 100 + 300 + (1300 * (speed - 8) if speed > 8 else 0)) / 1000


def random_step(speed):
    return 0.2 + speed * 0.1


def _random_hue(step, i):
    return ((step * 2654435761) ^ (i * 40503)) % 997 / 997


def effect_frame(effect, n, t, color, brightness, speed):
    """Cores de n LEDs no instante t (segundos) de um efeito animado."""
    value = brightness / 255
    if effect == "pulse":
        ramp = pulse_ramp(speed)
        x = t % (2 * ramp + 0.2)
        level = (1 - math.cos(math.pi * min(x, 2 * ramp) / ramp)) / 2 if x < 2 * ramp else 0.0
    elif effect == "flash":
        level = 1.0 if t % flash_period(speed) < 0.1 else 0.0
    elif effect == "cycle":
        hue = t / (cycle_step(speed) * CYCLE_COLORS) % 1
        rgb = colorsys.hsv_to_rgb(hue, 1, value)
        return [tuple(int(c * 255) for c in rgb)] * n
    elif effect == "random":
        step = int(t // random_step(speed))
        return [
            tuple(int(c * 255) for c in colorsys.hsv_to_rgb(_random_hue(step, i), 1, value))
            for i in range(n)
        ]
    else:
        raise ValueError(f"efeito sem animação: {effect}")
    k = value * level
    return [tuple(int(c * k) for c in color)] * n


# cores bem distintas entre vizinhos, para conferir LED por LED
CHECK_PALETTE = (
    ("vermelho", (255, 0, 0)),
    ("laranja", (255, 80, 0)),
    ("amarelo", (255, 200, 0)),
    ("verde", (0, 255, 0)),
    ("ciano", (0, 255, 255)),
    ("azul", (0, 0, 255)),
    ("roxo", (140, 0, 255)),
    ("rosa", (255, 0, 120)),
    ("branco", (255, 255, 255)),
    ("verde-limão", (120, 255, 0)),
    ("azul-claro", (0, 120, 255)),
    ("magenta", (255, 0, 255)),
)


def palette_colors(n):
    return [CHECK_PALETTE[i % len(CHECK_PALETTE)][1] for i in range(n)]


def light_first(c, z, n, colors=None):
    """Acende os primeiros n LEDs (branco, ou as cores dadas) e apaga o resto do chip."""
    if n > c.chip_led_count(z):
        c.set_chip_led_count(z, n)
    lit = colors or [(255, 255, 255)] * n
    c.set_colors(z, lit + [(0, 0, 0)] * (c.chip_led_count(z) - n))
