"""Padrões de cor usados pela CLI e pelo daemon: arco-íris e conferência de LEDs."""

import colorsys


def rainbow_frame(n, t, value):
    """Arco-íris de n LEDs com fase t (0-1)."""
    return [tuple(int(x * 255) for x in colorsys.hsv_to_rgb((t + i / n) % 1, 1, value)) for i in range(n)]


RAINBOW_FPS = 30
# As memórias recebem 1 de cada N quadros, sempre com a mesma fase: continuam
# sincronizadas com as fans e o SMBus (que o BIOS também usa) fica ocupado
# ~10% do tempo em vez de ~21%.
DRAM_FRAME_DIVIDER = 2


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
