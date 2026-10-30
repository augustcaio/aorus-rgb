//! Desenho do dispositivo com as zonas de LED, como o modo avançado do RGB
//! Fusion: cada zona é uma barra que mostra a cor e a animação atuais, e
//! clicar nela seleciona a zona.
//!
//! As posições dos headers na placa são ilustrativas (não há desenho técnico
//! da B550M AORUS ELITE aqui); a interface diz isso abaixo do desenho.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::f64::consts::PI;
use std::rc::Rc;
use std::time::Instant;

use gtk::cairo::Context;
use gtk::glib;
use gtk::prelude::*;

use super::theme::{DIM, GRAY, ORANGE};
use super::wheel::hsv_to_rgb;
use crate::model::{Device, Effect, Kind, Zone, ZoneState};

type Color = (f64, f64, f64);
type Rect = (f64, f64, f64, f64);

/// Fase do arco-íris: mesma fórmula do daemon (velocidade * 0,03 voltas/s).
const PHASE_PER_SECOND: f64 = 0.03;
const UNSET: Color = (0.10, 0.10, 0.10);

/// Cor de cada LED de uma zona no instante `t` (segundos).
pub fn led_colors(state: Option<&ZoneState>, leds: usize, t: f64) -> Vec<Color> {
    let leds = leds.max(1);
    let to_f64 = |c: crate::model::Rgb| c.to_f64();
    match state {
        None => vec![UNSET; leds],
        Some(ZoneState::Off) => vec![(0.06, 0.06, 0.06); leds],
        Some(ZoneState::Color(c)) => vec![to_f64(*c); leds],
        Some(ZoneState::Rainbow { speed, brightness }) => {
            let phase = (t * speed * PHASE_PER_SECOND).rem_euclid(1.0);
            let v = f64::from(*brightness) / 255.0;
            (0..leds)
                .map(|i| to_f64(hsv_to_rgb(phase + i as f64 / leds as f64, 1.0, v)))
                .collect()
        }
        Some(ZoneState::Effect {
            effect,
            color,
            brightness,
            speed,
        }) => {
            // velocidade do daemon: 0 rápido .. 9 lento
            let period = 0.6 + f64::from(*speed) * 0.25;
            let v = f64::from(*brightness) / 255.0;
            let (r, g, b) = to_f64(*color);
            let scale = |k: f64| (r * k * v, g * k * v, b * k * v);
            match effect {
                Effect::Pulse => vec![scale(0.5 - 0.5 * (t * 2.0 * PI / period).cos()); leds],
                Effect::Flash => vec![
                    scale(if (t / period).fract() < 0.15 {
                        1.0
                    } else {
                        0.0
                    });
                    leds
                ],
                Effect::Cycle => vec![to_f64(hsv_to_rgb(t / (period * 6.0), 1.0, v)); leds],
                Effect::Random => {
                    let step = (t / (period / 2.0)).floor() as u64;
                    (0..leds)
                        .map(|i| {
                            let hash = (step.wrapping_mul(2_654_435_761)
                                ^ (i as u64).wrapping_mul(40_503))
                                % 997;
                            to_f64(hsv_to_rgb(hash as f64 / 997.0, 1.0, v))
                        })
                        .collect()
                }
                _ => vec![scale(1.0); leds],
            }
        }
    }
}

fn animated(state: &HashMap<String, ZoneState>) -> bool {
    state.values().any(|s| match s {
        ZoneState::Rainbow { speed, .. } => *speed > 0.0,
        ZoneState::Effect { effect, .. } => *effect != Effect::Static,
        _ => false,
    })
}

pub struct Diagram {
    pub area: gtk::DrawingArea,
    device: Cell<Device>,
    zones: RefCell<Vec<Zone>>,
    state: RefCell<HashMap<String, ZoneState>>,
    selected: RefCell<HashSet<String>>,
    hits: RefCell<Vec<(String, Rect)>>,
    on_select: RefCell<Option<Box<dyn Fn()>>>,
    start: Instant,
}

impl Diagram {
    pub fn new() -> Rc<Self> {
        let area = gtk::DrawingArea::builder()
            .hexpand(true)
            .vexpand(true)
            .build();
        let diagram = Rc::new(Self {
            area: area.clone(),
            device: Cell::new(Device::Board),
            zones: RefCell::new(Vec::new()),
            state: RefCell::new(HashMap::new()),
            selected: RefCell::new(HashSet::new()),
            hits: RefCell::new(Vec::new()),
            on_select: RefCell::new(None),
            start: Instant::now(),
        });

        let weak = Rc::downgrade(&diagram);
        area.set_draw_func(move |_, cr, w, h| {
            if let Some(diagram) = weak.upgrade() {
                diagram.draw(cr, f64::from(w), f64::from(h));
            }
        });

        let click = gtk::GestureClick::new();
        let weak = Rc::downgrade(&diagram);
        click.connect_pressed(move |_, _, x, y| {
            if let Some(diagram) = weak.upgrade() {
                diagram.click(x, y);
            }
        });
        area.add_controller(click);

        let weak = Rc::downgrade(&diagram);
        area.add_tick_callback(move |area, _| match weak.upgrade() {
            Some(diagram) => {
                if animated(&diagram.state.borrow()) {
                    area.queue_draw();
                }
                glib::ControlFlow::Continue
            }
            None => glib::ControlFlow::Break,
        });
        diagram
    }

    pub fn connect_select(&self, f: impl Fn() + 'static) {
        self.on_select.replace(Some(Box::new(f)));
    }

    pub fn set_device(&self, device: Device) {
        self.device.set(device);
        self.selected.borrow_mut().clear();
        self.area.queue_draw();
    }

    pub fn set_zones(&self, zones: Vec<Zone>) {
        self.zones.replace(zones);
        self.area.queue_draw();
    }

    pub fn set_state(&self, state: HashMap<String, ZoneState>) {
        self.state.replace(state);
        self.area.queue_draw();
    }

    /// Zonas selecionadas, na ordem do dispositivo.
    pub fn selected(&self) -> Vec<String> {
        let selected = self.selected.borrow();
        self.zones
            .borrow()
            .iter()
            .filter(|z| selected.contains(&z.name))
            .map(|z| z.name.clone())
            .collect()
    }

    fn click(&self, x: f64, y: f64) {
        let hit = self
            .hits
            .borrow()
            .iter()
            .find(|(_, (rx, ry, rw, rh))| {
                x >= rx - 6.0 && x <= rx + rw + 6.0 && y >= ry - 6.0 && y <= ry + rh + 6.0
            })
            .map(|(name, _)| name.clone());
        {
            let mut selected = self.selected.borrow_mut();
            match hit {
                Some(name) if !selected.remove(&name) => {
                    selected.insert(name);
                }
                Some(_) => {}
                None => selected.clear(),
            }
        }
        self.area.queue_draw();
        if let Some(f) = self.on_select.borrow().as_ref() {
            f();
        }
    }

    fn draw(&self, cr: &Context, w: f64, h: f64) {
        self.hits.borrow_mut().clear();
        let t = self.start.elapsed().as_secs_f64();
        match self.device.get() {
            Device::Board => self.draw_board(cr, w, h, t),
            Device::Memory => self.draw_memory(cr, w, h, t),
        }
    }

    fn zone(&self, name: &str) -> Option<Zone> {
        self.zones.borrow().iter().find(|z| z.name == name).cloned()
    }

    /// Barra de LEDs de uma zona, com rótulo e marca de seleção.
    fn draw_zone(
        &self,
        cr: &Context,
        name: &str,
        label: &str,
        rect: Rect,
        t: f64,
        label_above: bool,
    ) {
        let Some(zone) = self.zone(name) else { return };
        let (x, y, w, h) = rect;
        let leds = if zone.kind == Kind::Rgb {
            1
        } else {
            zone.leds.max(1) as usize
        };
        let colors = led_colors(self.state.borrow().get(name), leds, t);
        let horizontal = w >= h;
        let unset = !self.state.borrow().contains_key(name);
        let n = colors.len() as f64;
        for (i, (r, g, b)) in colors.iter().enumerate() {
            let i = i as f64;
            cr.set_source_rgb(*r, *g, *b);
            if horizontal {
                cr.rectangle(x + w * i / n, y, w / n + 0.5, h);
            } else {
                cr.rectangle(x, y + h * i / n, w, h / n + 0.5);
            }
            let _ = cr.fill();
        }
        if unset {
            cr.set_source_rgb(GRAY.0, GRAY.1, GRAY.2);
            cr.set_line_width(1.0);
            cr.set_dash(&[3.0, 3.0], 0.0);
            cr.rectangle(x, y, w, h);
            let _ = cr.stroke();
            cr.set_dash(&[], 0.0);
        }
        let selected = self.selected.borrow().contains(name);
        if selected {
            cr.set_source_rgb(1.0, 1.0, 1.0);
            cr.set_line_width(2.0);
            cr.rectangle(x - 3.0, y - 3.0, w + 6.0, h + 6.0);
            let _ = cr.stroke();
        }
        // ícone de "sol", como o RGB Fusion marca cada zona
        let (sx, sy) = if horizontal {
            (x - 16.0, y + h / 2.0)
        } else {
            (x + w / 2.0, y - 16.0)
        };
        let (r, g, b) = if selected { (1.0, 1.0, 1.0) } else { ORANGE };
        cr.set_source_rgb(r, g, b);
        cr.set_line_width(1.4);
        cr.arc(sx, sy, 3.5, 0.0, 2.0 * PI);
        let _ = cr.stroke();
        for i in 0..8 {
            let a = f64::from(i) * PI / 4.0;
            cr.move_to(sx + a.cos() * 5.5, sy + a.sin() * 5.5);
            cr.line_to(sx + a.cos() * 8.0, sy + a.sin() * 8.0);
        }
        let _ = cr.stroke();
        // rótulo
        cr.select_font_face(
            "Sans",
            gtk::cairo::FontSlant::Normal,
            gtk::cairo::FontWeight::Bold,
        );
        cr.set_font_size(10.0);
        cr.set_source_rgb(0.94, 0.94, 0.94);
        let text = if zone.kind == Kind::Rgb {
            label.to_owned()
        } else {
            format!("{label} · {} LEDs", zone.leds)
        };
        let extents = cr.text_extents(&text).ok();
        let tw = extents.map_or(0.0, |e| e.width());
        let (lx, ly) = if horizontal {
            (
                x + (w - tw) / 2.0,
                if label_above { y - 8.0 } else { y + h + 16.0 },
            )
        } else {
            (x + (w - tw) / 2.0, y - 28.0)
        };
        cr.move_to(lx, ly);
        let _ = cr.show_text(&text);
        self.hits.borrow_mut().push((name.to_owned(), rect));
    }

    fn outline(cr: &Context, rect: Rect, color: Color, width: f64) {
        cr.set_source_rgb(color.0, color.1, color.2);
        cr.set_line_width(width);
        cr.rectangle(rect.0, rect.1, rect.2, rect.3);
        let _ = cr.stroke();
    }

    fn draw_board(&self, cr: &Context, w: f64, h: f64, t: f64) {
        // Micro-ATX: 244 x 244 mm, quase quadrada
        let side = (w - 64.0).min(h - 76.0).max(80.0);
        // sobra à direita para a barra do LED_CPU
        let (bx, by) = ((w - 30.0 - side) / 2.0, (h - side) / 2.0);
        let u = |fx: f64, fy: f64, fw: f64, fh: f64| {
            (bx + fx * side, by + fy * side, fw * side, fh * side)
        };
        cr.set_source_rgb(0.09, 0.09, 0.09);
        cr.rectangle(bx, by, side, side);
        let _ = cr.fill();
        Self::outline(cr, (bx, by, side, side), (0.42, 0.42, 0.42), 1.5);
        let part = (0.33, 0.33, 0.33);
        Self::outline(cr, u(0.02, 0.05, 0.08, 0.40), part, 1.0); // painel traseiro
        Self::outline(cr, u(0.22, 0.12, 0.28, 0.28), part, 1.0); // soquete
        Self::outline(cr, u(0.27, 0.17, 0.18, 0.18), part, 1.0);
        for i in 0..4 {
            Self::outline(
                cr,
                u(0.62 + f64::from(i) * 0.055, 0.07, 0.03, 0.46),
                part,
                1.0,
            ); // memória
        }
        for (i, len) in [0.62, 0.30, 0.62].into_iter().enumerate() {
            Self::outline(cr, u(0.06, 0.60 + i as f64 * 0.12, len, 0.035), part, 1.0); // PCIe
        }
        Self::outline(cr, u(0.70, 0.70, 0.14, 0.14), part, 1.0); // chipset
        Self::outline(cr, u(0.24, 0.66, 0.36, 0.03), part, 1.0); // M.2
        cr.arc(
            bx + 0.10 * side,
            by + 0.93 * side,
            0.025 * side,
            0.0,
            2.0 * PI,
        ); // bateria
        cr.set_source_rgb(part.0, part.1, part.2);
        let _ = cr.stroke();

        let bar = 6.0;
        let top = by - 18.0;
        let bottom = by + side + 12.0;
        self.draw_zone(
            cr,
            "led_c2",
            "LED_C2",
            (bx + 0.08 * side, top, 0.30 * side, bar),
            t,
            true,
        );
        self.draw_zone(
            cr,
            "d_led2",
            "D_LED2",
            (bx + 0.56 * side, top, 0.34 * side, bar),
            t,
            true,
        );
        self.draw_zone(
            cr,
            "led_c1",
            "LED_C1",
            (bx + 0.08 * side, bottom, 0.30 * side, bar),
            t,
            false,
        );
        self.draw_zone(
            cr,
            "d_led1",
            "D_LED1",
            (bx + 0.56 * side, bottom, 0.34 * side, bar),
            t,
            false,
        );
        self.draw_zone(
            cr,
            "led_cpu",
            "LED_CPU",
            (bx + side + 14.0, by + 0.14 * side, bar, 0.34 * side),
            t,
            false,
        );
    }

    fn draw_memory(&self, cr: &Context, w: f64, h: f64, t: f64) {
        let names: Vec<String> = self
            .zones
            .borrow()
            .iter()
            .filter(|z| z.kind == Kind::Dram)
            .map(|z| z.name.clone())
            .collect();
        if names.is_empty() {
            cr.set_source_rgb(GRAY.0, GRAY.1, GRAY.2);
            cr.select_font_face(
                "Sans",
                gtk::cairo::FontSlant::Normal,
                gtk::cairo::FontWeight::Normal,
            );
            cr.set_font_size(13.0);
            cr.move_to(24.0, h / 2.0);
            let _ = cr.show_text("Nenhuma memória RGB detectada pelo daemon");
            return;
        }
        let stick_w = (w - 40.0).clamp(140.0, 300.0);
        let stick_h = stick_w * 0.24;
        let gap = stick_h * 1.1;
        let total = names.len() as f64 * stick_h + (names.len() as f64 - 1.0) * gap;
        let x = (w - stick_w) / 2.0 + 8.0;
        let mut y = (h - total) / 2.0;
        for name in &names {
            // pente: placa de circuito, chips e contatos
            cr.set_source_rgb(0.09, 0.09, 0.09);
            cr.rectangle(x, y, stick_w, stick_h);
            let _ = cr.fill();
            Self::outline(cr, (x, y, stick_w, stick_h), (0.42, 0.42, 0.42), 1.5);
            for i in 0..8 {
                Self::outline(
                    cr,
                    (
                        x + stick_w * (0.05 + f64::from(i) * 0.115),
                        y + stick_h * 0.32,
                        stick_w * 0.08,
                        stick_h * 0.36,
                    ),
                    (0.30, 0.30, 0.30),
                    1.0,
                );
            }
            cr.set_source_rgb(DIM.0, DIM.1, DIM.2);
            cr.rectangle(x + stick_w * 0.47, y + stick_h - 6.0, stick_w * 0.03, 6.0);
            let _ = cr.fill();
            // faixa de LEDs no topo do pente
            let label = name.to_uppercase();
            self.draw_zone(
                cr,
                name,
                &label,
                (x + stick_w * 0.04, y - 12.0, stick_w * 0.92, 8.0),
                t,
                true,
            );
            y += stick_h + gap;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Rgb;

    #[test]
    fn rainbow_spans_the_hue_circle_and_moves_with_time() {
        let state = ZoneState::Rainbow {
            speed: 5.0,
            brightness: 255,
        };
        let at0 = led_colors(Some(&state), 3, 0.0);
        assert_eq!(at0[0], (1.0, 0.0, 0.0));
        assert_ne!(led_colors(Some(&state), 3, 2.0)[0], at0[0]);
    }

    #[test]
    fn static_color_and_off() {
        assert_eq!(
            led_colors(Some(&ZoneState::Color(Rgb(255, 0, 0))), 2, 9.0),
            vec![(1.0, 0.0, 0.0); 2]
        );
        assert!(led_colors(Some(&ZoneState::Off), 1, 0.0)[0].0 < 0.1);
    }

    #[test]
    fn pulse_goes_dark_and_bright() {
        let state = ZoneState::Effect {
            effect: Effect::Pulse,
            color: Rgb(255, 0, 0),
            brightness: 255,
            speed: 0,
        };
        let dark = led_colors(Some(&state), 1, 0.0)[0].0;
        let bright = led_colors(Some(&state), 1, 0.3)[0].0;
        assert!(dark < 0.01 && bright > 0.99);
    }
}
