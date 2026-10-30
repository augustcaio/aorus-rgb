//! Desenho do dispositivo com as zonas de LED, como o modo avançado do RGB
//! Fusion: cada zona é uma barra que mostra a cor e a animação atuais, e
//! clicar nela seleciona a zona.
//!
//! A placa segue o leiaute do manual da B550M AORUS ELITE: cada header de LED
//! fica na posição real e uma linha de chamada o liga à barra da zona. Os
//! desenhos são próprios, sem artes da Gigabyte.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::f64::consts::PI;
use std::rc::Rc;
use std::time::Instant;

use gtk::cairo::{Context, FontSlant, FontWeight, LinearGradient};
use gtk::glib;
use gtk::prelude::*;

use super::art;
use super::theme::{GRAY, LINE, ORANGE};
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
            // mesmos tempos que o daemon usa ao desenhar os efeitos
            // (patterns.py; velocidade 0 rápido .. 9 lento)
            let speed = f64::from(*speed);
            let v = f64::from(*brightness) / 255.0;
            let (r, g, b) = to_f64(*color);
            let scale = |k: f64| (r * k * v, g * k * v, b * k * v);
            match effect {
                Effect::Pulse => {
                    let ramp = pulse_ramp(speed);
                    let x = t.rem_euclid(2.0 * ramp + 0.2);
                    let level = if x < 2.0 * ramp {
                        (1.0 - (PI * x / ramp).cos()) / 2.0
                    } else {
                        0.0
                    };
                    vec![scale(level); leds]
                }
                Effect::Flash => {
                    let period = 0.2 + (speed * 200.0 + 700.0) / 1000.0;
                    let level = if t.rem_euclid(period) < 0.1 { 1.0 } else { 0.0 };
                    vec![scale(level); leds]
                }
                Effect::Cycle => {
                    let step = (speed * 100.0
                        + 300.0
                        + if speed > 8.0 {
                            1300.0 * (speed - 8.0)
                        } else {
                            0.0
                        })
                        / 1000.0;
                    vec![to_f64(hsv_to_rgb(t / (step * 7.0), 1.0, v)); leds]
                }
                Effect::Random => {
                    let step = (t / (0.2 + speed * 0.1)).floor() as u64;
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

/// Segundos para acender (e para apagar) no pulso.
fn pulse_ramp(speed: f64) -> f64 {
    if speed <= 6.0 {
        (400.0 + speed * 100.0) / 1000.0
    } else {
        (1000.0 + (speed - 6.0) * 200.0) / 1000.0
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
    /// Conectores marcados como vazios: aparecem só como o conector na placa.
    hidden: RefCell<HashSet<String>>,
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
            hidden: RefCell::new(HashSet::new()),
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

    pub fn set_hidden(&self, hidden: HashSet<String>) {
        self.selected
            .borrow_mut()
            .retain(|name| !hidden.contains(name));
        self.hidden.replace(hidden);
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

    /// Barra de LEDs de uma zona, com rótulo e marca de seleção. A cor é a
    /// única parte cheia do desenho: nas memórias, a barra de luz difusa;
    /// nos headers, um segmento por LED.
    fn draw_zone(&self, cr: &Context, name: &str, label: &str, rect: Rect, t: f64, side: Side) {
        let Some(zone) = self.zone(name) else { return };
        let (x, y, w, h) = rect;
        let leds = if zone.kind == Kind::Rgb {
            1
        } else {
            zone.leds.max(1) as usize
        };
        let state = self.state.borrow().get(name).cloned();
        let colors = led_colors(state.as_ref(), leds, t);
        let n = colors.len() as f64;
        if zone.kind == Kind::Dram {
            let gradient = LinearGradient::new(x, y, x + w, y);
            for (i, (r, g, b)) in colors.iter().enumerate() {
                gradient.add_color_stop_rgb((i as f64 + 0.5) / n, *r, *g, *b);
            }
            let _ = cr.set_source(&gradient);
            art::light_bar(cr, rect);
            let _ = cr.fill();
        } else {
            for (i, (r, g, b)) in colors.iter().enumerate() {
                let i = i as f64;
                cr.set_source_rgb(*r, *g, *b);
                cr.rectangle(x + w * i / n, y, w / n + 0.5, h);
                let _ = cr.fill();
            }
            // separação entre LEDs
            set(cr, BACKGROUND);
            cr.set_line_width(1.0);
            for i in 1..colors.len() {
                let lx = (x + w * i as f64 / n).round() + 0.5;
                cr.move_to(lx, y);
                cr.line_to(lx, y + h);
            }
            let _ = cr.stroke();
        }
        if state.is_none() {
            set(cr, GRAY);
            cr.set_line_width(1.0);
            cr.set_dash(&[3.0, 3.0], 0.0);
            cr.rectangle(x + 0.5, y + 0.5, w - 1.0, h - 1.0);
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

        // rótulo: nome em negrito e, nas zonas endereçáveis, a contagem de LEDs
        let baseline = match side {
            Side::Above => y - 7.0,
            Side::Below => y + h + 15.0,
        };
        cr.select_font_face("Sans", FontSlant::Normal, FontWeight::Bold);
        cr.set_font_size(10.0);
        set(cr, if selected { ORANGE } else { (0.94, 0.94, 0.94) });
        cr.move_to(x, baseline);
        let _ = cr.show_text(label);
        if zone.kind != Kind::Rgb {
            cr.select_font_face("Sans", FontSlant::Normal, FontWeight::Normal);
            set(cr, GRAY);
            let _ = cr.show_text(&format!(" · {} LEDs", zone.leds));
        }
        self.hits.borrow_mut().push((name.to_owned(), rect));
    }

    /// Header da zona na placa (contornado na cor atual), a linha de chamada
    /// e a barra fora da placa, que é o alvo do clique.
    fn draw_callout(&self, cr: &Context, callout: &Callout, t: f64) {
        let Some(zone) = self.zone(callout.name) else {
            return;
        };
        if self.hidden.borrow().contains(callout.name) {
            // nada ligado: só o conector vazio, no traço das outras peças
            let (hx, hy, hw, hh) = callout.header;
            cr.set_source_rgba(LINE.0, LINE.1, LINE.2, 0.55);
            cr.set_line_width(1.0);
            cr.rectangle(hx.round() + 0.5, hy.round() + 0.5, hw.round(), hh.round());
            let _ = cr.stroke();
            return;
        }
        let selected = self.selected.borrow().contains(callout.name);
        let state = self.state.borrow().get(callout.name).cloned();
        let lit = state.as_ref().is_some_and(|s| *s != ZoneState::Off);
        let color = led_colors(state.as_ref(), zone.leds.max(1) as usize, t)[0];

        set(cr, if selected { (1.0, 1.0, 1.0) } else { LINE });
        cr.set_line_width(1.0);
        for (i, (px, py)) in callout.path.iter().enumerate() {
            let (px, py) = (px.round() + 0.5, py.round() + 0.5);
            if i == 0 {
                cr.move_to(px, py);
            } else {
                cr.line_to(px, py);
            }
        }
        let _ = cr.stroke();

        let (hx, hy, hw, hh) = callout.header;
        set(
            cr,
            if selected {
                (1.0, 1.0, 1.0)
            } else if lit {
                color
            } else {
                LINE
            },
        );
        cr.set_line_width(1.5);
        cr.rectangle(hx.round() + 0.5, hy.round() + 0.5, hw.round(), hh.round());
        let _ = cr.stroke();
        self.hits
            .borrow_mut()
            .push((callout.name.to_owned(), callout.header));

        self.draw_zone(
            cr,
            callout.name,
            callout.label,
            callout.bar,
            t,
            callout.side,
        );
    }

    /// B550M AORUS ELITE em linhas, com cada header de LED na posição do
    /// manual ligado à barra da zona.
    fn draw_board(&self, cr: &Context, w: f64, h: f64, t: f64) {
        // a placa é quadrada; à esquerda fica a barra do LED_CPU e, acima e
        // abaixo, as barras dos headers das bordas
        const LEFT: f64 = 56.0;
        const ROW: f64 = 34.0;
        let s = (w - LEFT - 8.0).min(h - 2.0 * ROW).max(80.0).floor();
        let bx = (LEFT + ((w - LEFT - 8.0 - s) / 2.0).max(0.0)).round();
        let by = ((h - s) / 2.0).round();
        art::board(cr, (bx, by, s, s), LINE, art::Detail::Full);

        let r = |fx: f64, fy: f64, fw: f64, fh: f64| (bx + fx * s, by + fy * s, fw * s, fh * s);
        let p = |fx: f64, fy: f64| (bx + fx * s, by + fy * s);
        let top = by - 16.0;
        let bottom = by + s + 10.0;
        let bar = 6.0;
        let (c2x, c2y) = p(0.935, 0.135);
        let (d2x, d2y) = p(0.949, 0.174);
        let (c1x, c1y) = p(0.225, 0.905);
        let (d1x, d1y) = p(0.275, 0.905);
        let (cpux, cpuy) = p(0.245, 0.532);
        let callouts = [
            Callout {
                name: "led_c2",
                label: "LED_C2",
                header: r(0.915, 0.135, 0.04, 0.018),
                path: vec![
                    (c2x, c2y),
                    (c2x, by - 5.0),
                    (bx + 0.47 * s, by - 5.0),
                    (bx + 0.47 * s, top + bar),
                ],
                bar: (bx + 0.14 * s, top, 0.36 * s, bar),
                side: Side::Above,
            },
            Callout {
                name: "d_led2",
                label: "D_LED2",
                header: r(0.915, 0.165, 0.034, 0.018),
                path: vec![
                    (d2x, d2y),
                    (bx + 0.985 * s, d2y),
                    (bx + 0.985 * s, top + bar),
                ],
                bar: (bx + 0.56 * s, top, 0.44 * s, bar),
                side: Side::Above,
            },
            Callout {
                name: "led_cpu",
                label: "LED_CPU",
                header: r(0.245, 0.51, 0.018, 0.045),
                path: vec![(cpux, cpuy), (bx - 10.0, cpuy)],
                bar: (8.0, cpuy - bar / 2.0, bx - 18.0, bar),
                side: Side::Above,
            },
            Callout {
                name: "led_c1",
                label: "LED_C1",
                header: r(0.205, 0.885, 0.04, 0.02),
                path: vec![(c1x, c1y), (c1x, bottom)],
                bar: (bx, bottom, 0.33 * s, bar),
                side: Side::Below,
            },
            Callout {
                name: "d_led1",
                label: "D_LED1",
                header: r(0.258, 0.885, 0.034, 0.02),
                path: vec![
                    (d1x, d1y),
                    (d1x, by + s + 5.0),
                    (bx + 0.45 * s, by + s + 5.0),
                    (bx + 0.45 * s, bottom),
                ],
                bar: (bx + 0.42 * s, bottom, 0.44 * s, bar),
                side: Side::Below,
            },
        ];
        for callout in &callouts {
            self.draw_callout(cr, callout, t);
        }
    }

    /// Um pente em linhas por zona de memória; a barra de luz, no topo, é a
    /// zona.
    fn draw_memory(&self, cr: &Context, w: f64, h: f64, t: f64) {
        let names: Vec<String> = self
            .zones
            .borrow()
            .iter()
            .filter(|z| z.kind == Kind::Dram)
            .map(|z| z.name.clone())
            .collect();
        if names.is_empty() {
            set(cr, GRAY);
            cr.select_font_face("Sans", FontSlant::Normal, FontWeight::Normal);
            cr.set_font_size(13.0);
            cr.move_to(24.0, h / 2.0);
            let _ = cr.show_text("Nenhuma memória RGB detectada pelo daemon");
            return;
        }
        // largura que cabe nos dois sentidos, mantendo a proporção do pente
        const LABEL: f64 = 16.0;
        const GAP: f64 = 14.0;
        let count = names.len() as f64;
        let room = h - 8.0 - count * LABEL - (count - 1.0) * GAP;
        let sw = (w - 24.0)
            .min(300.0)
            .min(room / (count * art::MEMORY_ASPECT))
            .floor();
        let sh = sw * art::MEMORY_ASPECT;
        let total = count * (LABEL + sh) + (count - 1.0) * GAP;
        let x = ((w - sw) / 2.0).round();
        let mut y = ((h - total) / 2.0).round() + LABEL;
        for name in &names {
            let stick = (x, y, sw, sh);
            let label = name.to_uppercase();
            self.draw_zone(cr, name, &label, art::memory_bar(stick), t, Side::Above);
            art::memory(cr, stick, LINE, art::Detail::Full);
            y = (y + sh + GAP + LABEL).round();
        }
    }
}

/// Nome impresso na placa e posição de cada conector de LED, pelo manual.
pub fn connector(name: &str) -> Option<(&'static str, &'static str)> {
    Some(match name {
        "led_c1" => ("LED_C1", "borda de baixo, à esquerda"),
        "d_led1" => ("D_LED1", "borda de baixo, à esquerda"),
        "led_c2" => ("LED_C2", "canto superior direito"),
        "d_led2" => ("D_LED2", "canto superior direito"),
        "led_cpu" => ("LED_CPU", "ao lado do SYS_FAN1"),
        _ => return None,
    })
}

#[derive(Clone, Copy)]
enum Side {
    Above,
    Below,
}

/// Uma zona na placa: header, linha de chamada e barra clicável.
struct Callout<'a> {
    name: &'a str,
    label: &'a str,
    header: Rect,
    path: Vec<(f64, f64)>,
    bar: Rect,
    side: Side,
}

/// Fundo do diagrama, para separar os LEDs de uma barra.
const BACKGROUND: Color = (0.08, 0.08, 0.08);

fn set(cr: &Context, (r, g, b): Color) {
    cr.set_source_rgb(r, g, b);
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
        // velocidade 0: acende em 0,4 s, apaga em 0,4 s e fica 0,2 s apagado
        let bright = led_colors(Some(&state), 1, 0.4)[0].0;
        let off = led_colors(Some(&state), 1, 0.9)[0].0;
        assert!(dark < 0.01 && bright > 0.99 && off < 0.01);
    }
}
