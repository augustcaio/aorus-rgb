"""Driver do controlador ITE IT5702/IT8297 (RGB Fusion 2.0) via hidraw.

Protocolo baseado no driver do OpenRGB (GigabyteRGBFusion2USBController).
Tudo é feito com feature reports de 64 bytes, report id 0xCC.
"""

import fcntl
import glob
import os
import struct
import time

VID, PID = 0x048D, 0x5702
REPORT_ID = 0xCC
BUF = 64

# headers ARGB endereçáveis (D_LED1 = 0x58, D_LED2 = 0x59)
HDR_ARGB = {1: 0x58, 2: 0x59}
# bit de cada header no registrador de "efeito embutido desligado" (0x32)
HDR_BIT = {1: 0x01, 2: 0x02}

# tamanhos de strip aceitos pelo hardware (índice = código enviado)
LED_COUNT_STEPS = [32, 64, 256, 512, 1024]

EFFECTS = {
    "static": 1,
    "pulse": 2,
    "flash": 3,
    "cycle": 4,
    "wave": 6,
    "random": 8,
}

_IOC_WRITE, _IOC_READ = 1, 2


def _hid_ioc(nr, size):
    return ((_IOC_READ | _IOC_WRITE) << 30) | (size << 16) | (ord("H") << 8) | nr


HIDIOCSFEATURE = _hid_ioc(0x06, BUF)
HIDIOCGFEATURE = _hid_ioc(0x07, BUF)


class DeviceNotFound(Exception):
    pass


def find_hidraw():
    """Procura o hidraw do IT5702 via sysfs."""
    want = f"{VID:08X}:{PID:08X}"
    for uevent in glob.glob("/sys/class/hidraw/hidraw*/device/uevent"):
        with open(uevent) as f:
            if f"HID_ID=0003:{want[:8]}:{want[9:]}".upper() in f.read().upper():
                return "/dev/" + uevent.split("/")[4]
    raise DeviceNotFound("controlador ITE 048d:5702 não encontrado")


class Controller:
    def __init__(self, path=None):
        self.path = path or find_hidraw()
        self.fd = os.open(self.path, os.O_RDWR)
        self.effect_disabled = 0
        self.info = self._read_info()

    def close(self):
        os.close(self.fd)

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        self.close()

    # --- baixo nível -------------------------------------------------
    def _send(self, *data):
        buf = bytearray(BUF)
        buf[0] = REPORT_ID
        buf[1:1 + len(data)] = bytes(data)
        self._send_raw(buf)

    def _send_raw(self, buf):
        fcntl.ioctl(self.fd, HIDIOCSFEATURE, bytes(buf))

    def _get(self):
        buf = bytearray(BUF)
        buf[0] = REPORT_ID
        fcntl.ioctl(self.fd, HIDIOCGFEATURE, buf)
        return bytes(buf)

    def _read_info(self):
        self._send(0x60, 0x00)
        b = self._get()
        fw = struct.unpack_from("<I", b, 4)[0]
        counts = [b[8] & 0xF, b[8] >> 4, b[9] & 0xF, b[9] >> 4, b[10] & 0xF, b[10] >> 4]
        name = b[12:40].split(b"\0")[0].decode("ascii", "replace")
        # ordem de cor de cada header ARGB (3 bytes: índices de R, G, B)
        cal0, cal1 = struct.unpack_from("<II", b, 44)[0], struct.unpack_from("<I", b, 48)[0]
        return {
            "product": name,
            "fw": ".".join(str((fw >> s) & 0xFF) for s in (0, 8, 16, 24)),
            "device_num": b[2],
            "strip_detect": b[3],
            "argb_counts": [LED_COUNT_STEPS[min(c, 4)] for c in counts],
            "cal": [cal0, cal1],
            "raw": b,
        }

    # --- alto nível --------------------------------------------------
    def set_led_count(self, hdr, count):
        """Define o tamanho (arredondado p/ cima) do strip do header 1 ou 2."""
        code = next((i for i, n in enumerate(LED_COUNT_STEPS) if count <= n), 4)
        codes = [LED_COUNT_STEPS.index(c) for c in self.info["argb_counts"]]
        codes[(hdr - 1)] = code
        self._send(0x34, codes[1] << 4 | codes[0], codes[3] << 4 | codes[2], codes[5] << 4 | codes[4])
        self.info["argb_counts"][hdr - 1] = LED_COUNT_STEPS[code]

    def builtin_effects(self, hdr, enable):
        bit = HDR_BIT[hdr]
        self.effect_disabled = self.effect_disabled & ~bit if enable else self.effect_disabled | bit
        self._send(0x32, self.effect_disabled)
        time.sleep(0.05)

    def reset(self):
        for reg in range(0x20, 0x28):
            self._send(reg, 0, 0)
        self._send(0x28, 0xFF, 0x00)

    def _byte_order(self, hdr):
        cal = self.info["cal"][hdr - 1]
        return (cal >> 16) & 0xFF, (cal >> 8) & 0xFF, cal & 0xFF

    def set_strip(self, hdr, colors):
        """Envia cores diretas (lista de (r,g,b)) para um header ARGB."""
        bo_r, bo_g, bo_b = self._byte_order(hdr)
        sent = 0
        for i in range(0, len(colors), 19):
            chunk = colors[i:i + 19]
            buf = bytearray(BUF)
            buf[0], buf[1] = REPORT_ID, HDR_ARGB[hdr]
            struct.pack_into("<H", buf, 2, sent)
            buf[4] = len(chunk) * 3
            for j, (r, g, b) in enumerate(chunk):
                off = 5 + j * 3
                buf[off + bo_r], buf[off + bo_g], buf[off + bo_b] = r, g, b
            self._send_raw(buf)
            sent += len(chunk) * 3

    def set_effect(self, led, effect, color, brightness=255, speed=5):
        """Efeito de hardware nos LEDs 'comuns' (led 0..7). led=-1 = todos."""
        buf = bytearray(BUF)
        buf[0] = REPORT_ID
        if led == -1:
            zone, buf[1] = 0xFF, 0x20
        else:
            zone, buf[1] = 1 << led, 0x20 + led
        struct.pack_into("<I", buf, 2, zone)
        buf[11], buf[12], buf[13] = EFFECTS[effect], brightness, 0
        r, g, b = color
        struct.pack_into("<I", buf, 14, b | g << 8 | r << 16)
        if effect == "pulse":
            p = 400 + speed * 100 if speed <= 6 else 1000 + (speed - 6) * 200
            struct.pack_into("<HHH", buf, 22, p, p, 200)
        elif effect == "flash":
            struct.pack_into("<HHH", buf, 22, 100, 100, speed * 200 + 700)
        elif effect == "cycle":
            p = speed * 100 + 300
            struct.pack_into("<HH", buf, 22, p, p - 200)
            buf[30] = 7
        self._send_raw(buf)
        self._send(0x28, 0xFF, 0x00)
