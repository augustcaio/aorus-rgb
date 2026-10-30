//! Zonas, estados e efeitos, no mesmo formato da API D-Bus do daemon
//! (`docs/daemon.md` na branch `main`).

/// Tipo de zona informado por `GetZones`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Header ARGB endereçável da placa (`d_led1`, `d_led2`).
    Argb,
    /// Header RGB de 12 V ou LEDs da placa: uma cor só.
    Rgb,
    /// Pente de memória (controlador ENE).
    Dram,
}

impl Kind {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "argb" => Some(Self::Argb),
            "rgb" => Some(Self::Rgb),
            "dram" => Some(Self::Dram),
            _ => None,
        }
    }

    pub fn addressable(self) -> bool {
        self != Self::Rgb
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Zone {
    pub name: String,
    pub kind: Kind,
    pub leds: u32,
}

impl Zone {
    pub fn new(name: &str, kind: Kind, leds: u32) -> Self {
        Self {
            name: name.to_owned(),
            kind,
            leds,
        }
    }
}

/// Dispositivos mostrados na interface, como no RGB Fusion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Device {
    Board,
    Memory,
}

impl Device {
    pub const ALL: [Self; 2] = [Self::Board, Self::Memory];

    pub fn label(self) -> &'static str {
        match self {
            Self::Board => "B550M AORUS ELITE",
            Self::Memory => "Memórias RGB",
        }
    }

    pub fn owns(self, zone: &Zone) -> bool {
        match self {
            Self::Board => zone.kind != Kind::Dram,
            Self::Memory => zone.kind == Kind::Dram,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub const BLACK: Self = Self(0, 0, 0);

    pub fn parse_hex(value: &str) -> Option<Self> {
        let hex = value.trim().trim_start_matches('#');
        if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
        Some(Self(byte(0)?, byte(2)?, byte(4)?))
    }

    pub fn hex(self) -> String {
        format!("{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }

    pub fn scaled(self, brightness: u8) -> Self {
        let scale = |c: u8| (u16::from(c) * u16::from(brightness) / 255) as u8;
        Self(scale(self.0), scale(self.1), scale(self.2))
    }

    pub fn to_f64(self) -> (f64, f64, f64) {
        (
            f64::from(self.0) / 255.0,
            f64::from(self.1) / 255.0,
            f64::from(self.2) / 255.0,
        )
    }
}

/// Efeitos da coluna central da interface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    Static,
    Off,
    Pulse,
    Flash,
    Cycle,
    Random,
    Rainbow,
}

impl Effect {
    pub const ALL: [Self; 7] = [
        Self::Static,
        Self::Off,
        Self::Pulse,
        Self::Flash,
        Self::Cycle,
        Self::Random,
        Self::Rainbow,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Static => "Estático",
            Self::Off => "Desligado",
            Self::Pulse => "Pulso",
            Self::Flash => "Flash",
            Self::Cycle => "Ciclo de cores",
            Self::Random => "Aleatório",
            Self::Rainbow => "Arco-íris",
        }
    }

    /// Nome do efeito de hardware no daemon (`SetEffect`).
    pub fn daemon_name(self) -> Option<&'static str> {
        match self {
            Self::Pulse => Some("pulse"),
            Self::Flash => Some("flash"),
            Self::Cycle => Some("cycle"),
            Self::Random => Some("random"),
            Self::Static | Self::Off | Self::Rainbow => None,
        }
    }

    pub fn from_daemon_name(name: &str) -> Option<Self> {
        match name {
            "static" => Some(Self::Static),
            "pulse" => Some(Self::Pulse),
            "flash" => Some(Self::Flash),
            "cycle" => Some(Self::Cycle),
            "random" => Some(Self::Random),
            _ => None,
        }
    }

    pub fn uses_color(self) -> bool {
        matches!(self, Self::Static | Self::Pulse | Self::Flash)
    }

    pub fn uses_speed(self) -> bool {
        !matches!(self, Self::Static | Self::Off)
    }
}

/// Estado de uma zona, como em `GetState`.
#[derive(Clone, Debug, PartialEq)]
pub enum ZoneState {
    Color(Rgb),
    Effect {
        effect: Effect,
        color: Rgb,
        brightness: u8,
        speed: u8,
    },
    Rainbow {
        speed: f64,
        brightness: u8,
    },
    Off,
}

/// O que o usuário escolheu nos controles: efeito, cor, brilho e velocidade.
///
/// A velocidade vai de 1 (lenta) a 10 (rápida), como um slider. O daemon usa
/// 0 (rápido) a 9 (lento) nos efeitos de hardware e "maior é mais rápido" no
/// arco-íris; a conversão fica aqui.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub effect: Effect,
    pub color: Rgb,
    pub brightness: u8,
    pub speed: u8,
}

pub const SPEED_MIN: u8 = 1;
pub const SPEED_MAX: u8 = 10;

impl Default for Look {
    fn default() -> Self {
        Self {
            effect: Effect::Rainbow,
            color: Rgb(255, 102, 0),
            brightness: 255,
            speed: 5,
        }
    }
}

impl Look {
    pub fn effect_speed(self) -> u32 {
        u32::from(SPEED_MAX - self.speed.clamp(SPEED_MIN, SPEED_MAX))
    }

    pub fn rainbow_speed(self) -> f64 {
        f64::from(self.speed.clamp(SPEED_MIN, SPEED_MAX))
    }

    /// Controles que reproduzem o estado de uma zona (ao selecioná-la).
    pub fn from_state(state: &ZoneState, fallback: Self) -> Self {
        match *state {
            ZoneState::Color(color) => Self {
                effect: Effect::Static,
                color,
                brightness: 255,
                ..fallback
            },
            ZoneState::Effect {
                effect,
                color,
                brightness,
                speed,
            } => Self {
                effect,
                color,
                brightness,
                speed: SPEED_MAX - speed.min(SPEED_MAX - SPEED_MIN),
            },
            ZoneState::Rainbow { speed, brightness } => Self {
                effect: Effect::Rainbow,
                brightness,
                speed: speed
                    .round()
                    .clamp(f64::from(SPEED_MIN), f64::from(SPEED_MAX)) as u8,
                ..fallback
            },
            ZoneState::Off => Self {
                effect: Effect::Off,
                ..fallback
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_roundtrip() {
        assert_eq!(Rgb::parse_hex("#FF6600"), Some(Rgb(255, 102, 0)));
        assert_eq!(Rgb(255, 102, 0).hex(), "ff6600");
        assert_eq!(Rgb::parse_hex("fff"), None);
        assert_eq!(Rgb::parse_hex("gg0000"), None);
    }

    #[test]
    fn brightness_scales_color() {
        assert_eq!(Rgb(255, 128, 0).scaled(127), Rgb(127, 63, 0));
    }

    #[test]
    fn slider_speed_maps_to_daemon_speeds() {
        let look = |speed| Look {
            speed,
            ..Look::default()
        };
        assert_eq!(look(1).effect_speed(), 9); // lenta
        assert_eq!(look(10).effect_speed(), 0); // rápida
        assert_eq!(look(5).effect_speed(), 5);
        assert!((look(7).rainbow_speed() - 7.0).abs() < f64::EPSILON);
    }

    #[test]
    fn look_from_state_inverts_effect_speed() {
        let state = ZoneState::Effect {
            effect: Effect::Pulse,
            color: Rgb(1, 2, 3),
            brightness: 200,
            speed: 0,
        };
        let look = Look::from_state(&state, Look::default());
        assert_eq!(
            (look.effect, look.speed, look.brightness),
            (Effect::Pulse, 10, 200)
        );
    }

    #[test]
    fn device_owns_its_zones() {
        assert!(Device::Board.owns(&Zone::new("led_c1", Kind::Rgb, 1)));
        assert!(Device::Memory.owns(&Zone::new("dram1", Kind::Dram, 8)));
        assert!(!Device::Board.owns(&Zone::new("dram1", Kind::Dram, 8)));
    }
}
