//! Fala com o daemon pelo D-Bus ou, com `--mock`, simula o hardware.
//!
//! A simulação segue as regras do daemon (grupos `all` e `dram`, arco-íris só
//! em zonas endereçáveis), para a interface se comportar igual nos dois modos.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gtk::gio;
use gtk::glib;
use gtk::prelude::*;

use crate::model::{Effect, Kind, Look, Rgb, Zone, ZoneState};

pub const BUS_NAME: &str = "io.github.augustcaio.AorusRGB";
pub const OBJECT_PATH: &str = "/io/github/augustcaio/AorusRGB";
pub const INTERFACE: &str = "io.github.augustcaio.AorusRGB1";
const ERROR_PREFIX: &str = "GDBus.Error:io.github.augustcaio.AorusRGB.Error: ";
const TIMEOUT_MS: i32 = 10_000;

pub type Listener = Rc<dyn Fn()>;

#[derive(Clone)]
pub enum Backend {
    Mock(Rc<RefCell<Mock>>),
    Dbus(Rc<Dbus>),
}

impl Backend {
    pub fn mock() -> Self {
        Self::Mock(Rc::new(RefCell::new(Mock::new())))
    }

    pub async fn dbus() -> Result<Self, String> {
        let connection = gio::bus_get_future(gio::BusType::Session)
            .await
            .map_err(|e| e.to_string())?;
        Ok(Self::Dbus(Rc::new(Dbus {
            connection,
            subscription: RefCell::new(None),
        })))
    }

    pub fn is_mock(&self) -> bool {
        matches!(self, Self::Mock(_))
    }

    pub async fn zones(&self) -> Result<Vec<Zone>, String> {
        match self {
            Self::Mock(mock) => Ok(mock.borrow().zones.clone()),
            Self::Dbus(dbus) => dbus.zones().await,
        }
    }

    pub async fn state(&self) -> Result<HashMap<String, ZoneState>, String> {
        match self {
            Self::Mock(mock) => Ok(mock.borrow().state.clone()),
            Self::Dbus(dbus) => dbus.state().await,
        }
    }

    /// Aplica o visual escolhido a uma zona ou grupo (`all`, `dram`).
    pub async fn apply(&self, target: &str, look: &Look) -> Result<(), String> {
        match self {
            Self::Mock(mock) => {
                let result = mock.borrow_mut().apply(target, look);
                if result.is_ok() {
                    Mock::notify(mock);
                }
                result
            }
            Self::Dbus(dbus) => dbus.apply(target, look).await,
        }
    }

    /// Recebe o aviso de mudança de estado (sinal `StateChanged`).
    pub fn connect_changed(&self, listener: Listener) {
        match self {
            Self::Mock(mock) => mock.borrow_mut().listeners.push(listener),
            Self::Dbus(dbus) => dbus.subscribe(listener),
        }
    }
}

// --- simulação ------------------------------------------------------------------

pub struct Mock {
    zones: Vec<Zone>,
    state: HashMap<String, ZoneState>,
    listeners: Vec<Listener>,
}

impl Mock {
    /// Mesmo hardware validado: 2 headers ARGB calibrados em 12 LEDs, 3 zonas RGB
    /// e 2 pentes ENE de 8 LEDs, começando no arco-íris como o daemon.
    fn new() -> Self {
        let zones = vec![
            Zone::new("d_led1", Kind::Argb, 12),
            Zone::new("d_led2", Kind::Argb, 12),
            Zone::new("led_c1", Kind::Rgb, 1),
            Zone::new("led_cpu", Kind::Rgb, 1),
            Zone::new("led_c2", Kind::Rgb, 1),
            Zone::new("dram1", Kind::Dram, 8),
            Zone::new("dram2", Kind::Dram, 8),
        ];
        let state = zones
            .iter()
            .filter(|z| z.kind.addressable())
            .map(|z| {
                (
                    z.name.clone(),
                    ZoneState::Rainbow {
                        speed: 5.0,
                        brightness: 255,
                    },
                )
            })
            .collect();
        Self {
            zones,
            state,
            listeners: Vec::new(),
        }
    }

    fn resolve(&self, target: &str) -> Result<Vec<Zone>, String> {
        let zones: Vec<Zone> = match target {
            "all" => self.zones.clone(),
            "dram" => self
                .zones
                .iter()
                .filter(|z| z.kind == Kind::Dram)
                .cloned()
                .collect(),
            name => self
                .zones
                .iter()
                .filter(|z| z.name == name)
                .cloned()
                .collect(),
        };
        if zones.is_empty() && target != "dram" {
            return Err(format!("zona desconhecida: {target}"));
        }
        Ok(zones)
    }

    fn apply(&mut self, target: &str, look: &Look) -> Result<(), String> {
        let mut zones = self.resolve(target)?;
        if look.effect == Effect::Rainbow {
            zones.retain(|z| z.kind.addressable());
            if zones.is_empty() {
                return Err(format!(
                    "{target} não é endereçável: o arco-íris precisa de LEDs individuais"
                ));
            }
        }
        let state = state_for(look);
        for zone in zones {
            self.state.insert(zone.name, state.clone());
        }
        Ok(())
    }

    fn notify(mock: &Rc<RefCell<Self>>) {
        let listeners = mock.borrow().listeners.clone();
        for listener in listeners {
            listener();
        }
    }
}

/// O estado que o daemon guarda para um visual (mesma conversão do `apply` real).
pub fn state_for(look: &Look) -> ZoneState {
    match look.effect {
        Effect::Off => ZoneState::Off,
        Effect::Static => ZoneState::Color(look.color.scaled(look.brightness)),
        Effect::Rainbow => ZoneState::Rainbow {
            speed: look.rainbow_speed(),
            brightness: look.brightness,
        },
        effect => ZoneState::Effect {
            effect,
            color: look.color,
            brightness: look.brightness,
            speed: look.effect_speed() as u8,
        },
    }
}

// --- D-Bus ------------------------------------------------------------------------

pub struct Dbus {
    connection: gio::DBusConnection,
    subscription: RefCell<Option<gio::SignalSubscription>>,
}

impl Dbus {
    async fn call(
        &self,
        method: &str,
        params: Option<glib::Variant>,
    ) -> Result<glib::Variant, String> {
        self.connection
            .call_future(
                Some(BUS_NAME),
                OBJECT_PATH,
                INTERFACE,
                method,
                params.as_ref(),
                None,
                gio::DBusCallFlags::NONE,
                TIMEOUT_MS,
            )
            .await
            .map_err(|e| e.message().trim_start_matches(ERROR_PREFIX).to_owned())
    }

    async fn zones(&self) -> Result<Vec<Zone>, String> {
        let reply = self.call("GetZones", None).await?;
        let mut zones = Vec::new();
        for entry in reply.child_value(0).iter() {
            let name = entry.child_value(0).str().unwrap_or_default().to_owned();
            let kind = entry.child_value(1).str().and_then(Kind::parse);
            let leds = entry.child_value(2).get::<u32>().unwrap_or(1);
            if let Some(kind) = kind {
                zones.push(Zone { name, kind, leds });
            }
        }
        Ok(zones)
    }

    async fn state(&self) -> Result<HashMap<String, ZoneState>, String> {
        let reply = self.call("GetState", None).await?;
        let mut state = HashMap::new();
        for entry in reply.child_value(0).iter() {
            let name = entry.child_value(0).str().unwrap_or_default().to_owned();
            let fields: HashMap<String, glib::Variant> = entry
                .child_value(1)
                .iter()
                .filter_map(|f| {
                    Some((
                        f.child_value(0).str()?.to_owned(),
                        f.child_value(1).as_variant()?,
                    ))
                })
                .collect();
            if let Some(zone_state) = parse_state(&fields) {
                state.insert(name, zone_state);
            }
        }
        Ok(state)
    }

    async fn apply(&self, target: &str, look: &Look) -> Result<(), String> {
        match look.effect {
            Effect::Off => self.call("Off", Some((target,).to_variant())).await,
            Effect::Static => {
                let color = look.color.scaled(look.brightness).hex();
                self.call("SetColor", Some((target, color).to_variant()))
                    .await
            }
            Effect::Rainbow => {
                let params = (target, look.rainbow_speed(), u32::from(look.brightness));
                self.call("SetRainbow", Some(params.to_variant())).await
            }
            effect => {
                let name = effect.daemon_name().expect("efeito de hardware");
                let params = (
                    target,
                    name,
                    look.color.hex(),
                    u32::from(look.brightness),
                    look.effect_speed(),
                );
                self.call("SetEffect", Some(params.to_variant())).await
            }
        }
        .map(|_| ())
    }

    fn subscribe(&self, listener: Listener) {
        let subscription = self.connection.subscribe_to_signal(
            Some(BUS_NAME),
            Some(INTERFACE),
            Some("StateChanged"),
            Some(OBJECT_PATH),
            None,
            gio::DBusSignalFlags::NONE,
            move |_| listener(),
        );
        self.subscription.replace(Some(subscription));
    }
}

fn parse_state(fields: &HashMap<String, glib::Variant>) -> Option<ZoneState> {
    let text = |key: &str| fields.get(key)?.str().map(str::to_owned);
    let unsigned = |key: &str| fields.get(key)?.get::<u32>();
    let color = |key: &str| Rgb::parse_hex(&text(key)?);
    match text("mode")?.as_str() {
        "color" => Some(ZoneState::Color(color("color")?)),
        "effect" => Some(ZoneState::Effect {
            effect: Effect::from_daemon_name(&text("effect")?)?,
            color: color("color")?,
            brightness: unsigned("brightness")?.min(255) as u8,
            speed: unsigned("speed")?.min(9) as u8,
        }),
        "rainbow" => Some(ZoneState::Rainbow {
            speed: fields.get("speed")?.get::<f64>()?,
            brightness: unsigned("brightness")?.min(255) as u8,
        }),
        "off" => Some(ZoneState::Off),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn look(effect: Effect) -> Look {
        Look {
            effect,
            ..Look::default()
        }
    }

    #[test]
    fn mock_starts_like_the_daemon() {
        let mock = Mock::new();
        assert_eq!(mock.zones.len(), 7);
        assert!(matches!(mock.state["dram1"], ZoneState::Rainbow { .. }));
        assert!(!mock.state.contains_key("led_c1"));
    }

    #[test]
    fn mock_rainbow_skips_single_color_zones_in_groups() {
        let mut mock = Mock::new();
        mock.apply("all", &look(Effect::Off)).unwrap();
        mock.apply("all", &look(Effect::Rainbow)).unwrap();
        assert_eq!(mock.state["led_c1"], ZoneState::Off);
        assert!(matches!(mock.state["d_led1"], ZoneState::Rainbow { .. }));
        assert!(mock.apply("led_c1", &look(Effect::Rainbow)).is_err());
    }

    #[test]
    fn mock_rejects_unknown_zones() {
        assert!(Mock::new().apply("dram9", &look(Effect::Off)).is_err());
    }

    #[test]
    fn static_color_bakes_in_the_brightness() {
        let look = Look {
            effect: Effect::Static,
            color: Rgb(200, 100, 0),
            brightness: 128,
            ..Look::default()
        };
        assert_eq!(state_for(&look), ZoneState::Color(Rgb(100, 50, 0)));
    }

    #[test]
    fn parses_daemon_state() {
        let fields: HashMap<String, glib::Variant> = [
            ("mode".to_owned(), "effect".to_variant()),
            ("effect".to_owned(), "pulse".to_variant()),
            ("color".to_owned(), "00ffff".to_variant()),
            ("brightness".to_owned(), 200u32.to_variant()),
            ("speed".to_owned(), 3u32.to_variant()),
        ]
        .into_iter()
        .collect();
        assert_eq!(
            parse_state(&fields),
            Some(ZoneState::Effect {
                effect: Effect::Pulse,
                color: Rgb(0, 255, 255),
                brightness: 200,
                speed: 3
            })
        );
    }
}
