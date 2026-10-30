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

use gtk::cairo::{Context, FontSlant, FontWeight, LinearGradient, RadialGradient};
use gtk::glib;
use gtk::prelude::*;

use super::theme::{GRAY, ORANGE};
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

    /// Barra de LEDs de uma zona, com rótulo e marca de seleção. Nas memórias
    /// a barra é difusa (os LEDs ficam atrás de um difusor); nos headers, cada
    /// LED aparece como um segmento.
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

        // halo da luz, na média das cores (o pente já tem o difusor)
        if let Some(state) = &state
            && *state != ZoneState::Off
            && zone.kind != Kind::Dram
        {
            let (r, g, b) = average(&colors);
            cr.set_source_rgba(r, g, b, 0.22);
            rounded(cr, (x - 3.0, y - 3.0, w + 6.0, h + 6.0), 3.0);
            let _ = cr.fill();
        }
        if zone.kind == Kind::Dram {
            let gradient = LinearGradient::new(x, y, x + w, y);
            for (i, (r, g, b)) in colors.iter().enumerate() {
                gradient.add_color_stop_rgb((i as f64 + 0.5) / n, *r, *g, *b);
            }
            let _ = cr.set_source(&gradient);
            light_bar(cr, rect);
            let _ = cr.fill();
            // difusor leitoso por cima
            let gloss = LinearGradient::new(x, y, x, y + h);
            gloss.add_color_stop_rgba(0.0, 1.0, 1.0, 1.0, 0.30);
            gloss.add_color_stop_rgba(0.5, 1.0, 1.0, 1.0, 0.0);
            let _ = cr.set_source(&gloss);
            light_bar(cr, rect);
            let _ = cr.fill();
        } else {
            for (i, (r, g, b)) in colors.iter().enumerate() {
                let i = i as f64;
                cr.set_source_rgb(*r, *g, *b);
                cr.rectangle(x + w * i / n, y, w / n + 0.5, h);
                let _ = cr.fill();
            }
            // separação entre LEDs
            set(cr, PART);
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
            set(cr, (0.55, 0.55, 0.55));
            let _ = cr.show_text(&format!(" · {} LEDs", zone.leds));
        }
        self.hits.borrow_mut().push((name.to_owned(), rect));
    }

    /// Header da zona na placa (aceso na cor atual), a linha de chamada e a
    /// barra fora da placa, que é o alvo do clique.
    fn draw_callout(&self, cr: &Context, callout: &Callout, t: f64) {
        let Some(zone) = self.zone(callout.name) else {
            return;
        };
        let selected = self.selected.borrow().contains(callout.name);
        let state = self.state.borrow().get(callout.name).cloned();
        let lit = state.as_ref().is_some_and(|s| *s != ZoneState::Off);
        let color = led_colors(state.as_ref(), zone.leds.max(1) as usize, t)[0];

        // linha de chamada: do header até a barra
        set(cr, if selected { (1.0, 1.0, 1.0) } else { LEADER });
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

        // header aceso
        let (hx, hy, hw, hh) = callout.header;
        if lit {
            let (cx, cy) = (hx + hw / 2.0, hy + hh / 2.0);
            let radius = hw.max(hh) * 1.4;
            let glow = RadialGradient::new(cx, cy, 0.0, cx, cy, radius);
            glow.add_color_stop_rgba(0.0, color.0, color.1, color.2, 0.55);
            glow.add_color_stop_rgba(1.0, color.0, color.1, color.2, 0.0);
            let _ = cr.set_source(&glow);
            cr.arc(cx, cy, radius, 0.0, 2.0 * PI);
            let _ = cr.fill();
        }
        set(cr, if lit { color } else { (0.20, 0.20, 0.20) });
        cr.rectangle(hx, hy, hw, hh);
        let _ = cr.fill();
        pin_grid(cr, callout.header, PART);
        if selected {
            cr.set_source_rgb(1.0, 1.0, 1.0);
            cr.set_line_width(1.0);
            cr.rectangle(hx - 1.5, hy - 1.5, hw + 3.0, hh + 3.0);
            let _ = cr.stroke();
        }
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

    /// B550M AORUS ELITE vista de cima, com o painel traseiro à esquerda. As
    /// posições seguem o leiaute da página 4 do manual, em frações da placa.
    fn draw_board(&self, cr: &Context, w: f64, h: f64, t: f64) {
        // Micro-ATX: 244 x 244 mm. À esquerda fica a barra do LED_CPU; acima
        // e abaixo, as barras dos headers das bordas.
        const LEFT: f64 = 56.0;
        const ROW: f64 = 34.0;
        let s = (w - LEFT - 8.0).min(h - 2.0 * ROW).max(80.0);
        let bx = LEFT + ((w - LEFT - 8.0 - s) / 2.0).max(0.0);
        let by = (h - s) / 2.0;
        let r = |fx: f64, fy: f64, fw: f64, fh: f64| (bx + fx * s, by + fy * s, fw * s, fh * s);
        let p = |fx: f64, fy: f64| (bx + fx * s, by + fy * s);

        // placa de circuito
        set(cr, PCB);
        rounded(cr, (bx, by, s, s), 3.0);
        let _ = cr.fill();
        set(cr, EDGE);
        cr.set_line_width(1.0);
        rounded(cr, (bx + 0.5, by + 0.5, s - 1.0, s - 1.0), 3.0);
        let _ = cr.stroke();
        // trilha que isola a seção de áudio, como nas placas AORUS
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.07);
        cr.set_dash(&[2.0, 2.0], 0.0);
        let (ax, ay) = p(0.0, 0.645);
        let (ex, ey) = p(0.17, 0.95);
        cr.move_to(ax + 2.0, ay);
        cr.line_to(ex, ay);
        cr.line_to(ex, ey);
        let _ = cr.stroke();
        cr.set_dash(&[], 0.0);
        for (fx, fy) in [
            (0.68, 0.04),
            (0.905, 0.04),
            (0.035, 0.62),
            (0.905, 0.60),
            (0.035, 0.965),
            (0.905, 0.965),
        ] {
            let (cx, cy) = p(fx, fy);
            set(cr, (0.22, 0.22, 0.22));
            cr.arc(cx, cy, 0.013 * s, 0.0, 2.0 * PI);
            let _ = cr.stroke();
        }

        // painel traseiro: cobertura sobre as portas de cima e portas expostas
        let shroud = [
            p(0.0, 0.02),
            p(0.13, 0.02),
            p(0.13, 0.40),
            p(0.10, 0.47),
            p(0.0, 0.47),
        ];
        metal(cr, r(0.0, 0.02, 0.13, 0.45));
        polygon(cr, &shroud);
        let _ = cr.fill_preserve();
        set(cr, EDGE);
        let _ = cr.stroke();
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.5);
        cr.set_line_width(1.5);
        for k in [0.0, 0.04, 0.08] {
            let (x0, y0) = p(0.02, 0.33 + k);
            let (x1, y1) = p(0.11, 0.24 + k);
            cr.move_to(x0, y0);
            cr.line_to(x1, y1);
        }
        let _ = cr.stroke();
        block(cr, r(0.0, 0.48, 0.085, 0.045), (0.13, 0.13, 0.13)); // U32_2
        block(cr, r(0.0, 0.535, 0.085, 0.05), (0.13, 0.13, 0.13)); // áudio
        for k in 0..3 {
            let (cx, cy) = p(0.015 + f64::from(k) * 0.027, 0.56);
            set(cr, (0.30, 0.30, 0.30));
            cr.arc(cx + 0.012 * s, cy, 0.008 * s, 0.0, 2.0 * PI);
            let _ = cr.fill();
        }

        // dissipadores do VRM, à esquerda e acima do soquete
        heatsink(cr, r(0.145, 0.10, 0.085, 0.37), 7, false);
        heatsink(cr, r(0.27, 0.055, 0.30, 0.06), 10, true);

        // soquete AM4 com o módulo de retenção
        for fy in [0.125, 0.43] {
            block(cr, r(0.34, fy, 0.26, 0.06), (0.12, 0.12, 0.12));
            for fx in [0.355, 0.585] {
                let (cx, cy) = p(fx, fy + 0.03);
                set(cr, (0.40, 0.40, 0.40));
                cr.arc(cx, cy, 0.008 * s, 0.0, 2.0 * PI);
                let _ = cr.fill();
            }
        }
        let socket = r(0.38, 0.215, 0.185, 0.195);
        block(cr, socket, (0.36, 0.36, 0.35));
        set(cr, (0.24, 0.24, 0.24));
        let (sx, sy, sw, sh) = socket;
        let step = (s * 0.014).max(2.0);
        let mut yy = sy + step;
        while yy < sy + sh - step / 2.0 {
            let mut xx = sx + step;
            while xx < sx + sw - step / 2.0 {
                let center = (xx - (sx + sw / 2.0)).abs() < sw * 0.18
                    && (yy - (sy + sh / 2.0)).abs() < sh * 0.18;
                if !center {
                    cr.rectangle(xx - 0.5, yy - 0.5, 1.0, 1.0);
                }
                xx += step;
            }
            yy += step;
        }
        let _ = cr.fill();
        block(cr, r(0.57, 0.20, 0.012, 0.22), (0.30, 0.30, 0.30)); // alavanca

        // quatro slots DDR4 com as travas nas pontas
        for i in 0..4 {
            let fx = 0.717 + f64::from(i) * 0.032;
            block(cr, r(fx, 0.04, 0.022, 0.545), (0.12, 0.12, 0.12));
            set(cr, PART);
            let (gx, gy, gw, gh) = r(fx + 0.008, 0.06, 0.006, 0.505);
            cr.rectangle(gx, gy, gw, gh);
            let _ = cr.fill();
            for fy in [0.03, 0.575] {
                block(cr, r(fx - 0.002, fy, 0.026, 0.02), (0.30, 0.30, 0.30));
            }
        }

        // alimentação, ventoinhas e bateria
        pins(cr, r(0.18, 0.018, 0.07, 0.045)); // ATX_12V
        pins(cr, r(0.94, 0.19, 0.045, 0.185)); // ATX
        pins(cr, r(0.61, 0.035, 0.05, 0.02)); // CPU_FAN
        pins(cr, r(0.18, 0.51, 0.02, 0.045)); // SYS_FAN1
        pins(cr, r(0.79, 0.885, 0.045, 0.02)); // SYS_FAN2
        let (cx, cy) = p(0.917, 0.437);
        let radius = 0.032 * s;
        let coin = RadialGradient::new(cx - radius / 3.0, cy - radius / 3.0, 0.0, cx, cy, radius);
        coin.add_color_stop_rgb(0.0, 0.70, 0.70, 0.72);
        coin.add_color_stop_rgb(1.0, 0.32, 0.32, 0.34);
        let _ = cr.set_source(&coin);
        cr.arc(cx, cy, radius, 0.0, 2.0 * PI);
        let _ = cr.fill();

        // M.2 de cima com dissipador; o de baixo exposto
        set(cr, (0.22, 0.22, 0.22));
        let (cx, cy) = p(0.165, 0.62);
        cr.arc(cx, cy, 0.007 * s, 0.0, 2.0 * PI);
        let _ = cr.fill();
        heatsink(cr, r(0.27, 0.597, 0.31, 0.045), 3, false);
        block(cr, r(0.585, 0.595, 0.02, 0.05), PART);
        block(cr, r(0.64, 0.745, 0.018, 0.04), PART);
        for fx in [0.31, 0.39, 0.455] {
            let (cx, cy) = p(fx, 0.765);
            set(cr, (0.22, 0.22, 0.22));
            cr.arc(cx, cy, 0.007 * s, 0.0, 2.0 * PI);
            let _ = cr.fill();
        }

        // PCIe: x16 com reforço metálico, x1 e x4
        slot(cr, r(0.17, 0.68, 0.41, 0.025), true);
        slot(cr, r(0.17, 0.76, 0.10, 0.025), false);
        slot(cr, r(0.17, 0.835, 0.41, 0.025), false);

        // chipset B550 sob o dissipador, SATA na borda direita
        heatsink(cr, r(0.70, 0.70, 0.165, 0.13), 0, false);
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.45);
        cr.set_line_width(1.5);
        let (x0, y0) = p(0.72, 0.81);
        let (x1, y1) = p(0.80, 0.72);
        cr.move_to(x0, y0);
        cr.line_to(x1, y1);
        let _ = cr.stroke();
        for fy in [0.70, 0.79] {
            block(cr, r(0.925, fy, 0.06, 0.075), (0.12, 0.12, 0.12));
            for k in [0.015, 0.045] {
                set(cr, PART);
                let (lx, ly, lw, lh) = r(0.935, fy + k, 0.04, 0.012);
                cr.rectangle(lx, ly, lw, lh);
                let _ = cr.fill();
            }
        }

        // chips: rede, áudio, Super I/O e BIOS
        for rect in [
            r(0.06, 0.685, 0.035, 0.035),
            r(0.09, 0.815, 0.045, 0.045),
            r(0.44, 0.79, 0.05, 0.05),
            r(0.37, 0.52, 0.03, 0.03),
        ] {
            block(cr, rect, (0.10, 0.10, 0.10));
            set(cr, (0.35, 0.35, 0.35));
            cr.arc(rect.0 + 3.0, rect.1 + 3.0, 0.8, 0.0, 2.0 * PI);
            let _ = cr.fill();
        }

        // headers da borda de baixo
        for rect in [
            r(0.09, 0.885, 0.035, 0.025), // F_AUDIO
            r(0.31, 0.885, 0.05, 0.025),  // TPM
            r(0.37, 0.885, 0.05, 0.025),  // COM
            r(0.48, 0.885, 0.04, 0.025),  // F_USB1
            r(0.58, 0.885, 0.07, 0.025),  // F_U32
            r(0.69, 0.885, 0.08, 0.025),  // F_PANEL
        ] {
            pins(cr, rect);
        }

        // zonas: header real, chamada e barra
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

    /// Pentes de memória RGB no formato da Asgard Loki W2: barra de luz com
    /// os cantos chanfrados e um rebaixo no meio, dissipador com textura de
    /// losangos que deixa a borda da PCB à mostra embaixo, e os contatos
    /// dourados com o chanfro do DDR4.
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
        // proporções medidas na foto do pente (largura = 1)
        let sw = (w - 24.0).clamp(140.0, 300.0);
        let bar_h = (sw * 0.047).round();
        let spreader_h = (sw * 0.23).round();
        let pcb_h = 4.0;
        let contact_h = 7.0;
        let label_h = 16.0;
        let stick_h = label_h + bar_h + spreader_h + pcb_h + contact_h;
        let gap = 14.0;
        let count = names.len() as f64;
        let total = count * stick_h + (count - 1.0) * gap;
        let x = ((w - sw) / 2.0).round();
        let mut y = ((h - total) / 2.0).round() + label_h;
        for name in &names {
            let top = y + bar_h;
            let bottom = top + spreader_h;
            let inset = (sw * 0.022).round();
            let shoulder = top + spreader_h * 0.33;

            // PCB: aparece nas laterais, abaixo do dissipador, e sobre os contatos
            set(cr, (0.05, 0.05, 0.05));
            cr.rectangle(x + 2.0, shoulder, sw - 4.0, bottom + pcb_h - shoulder);
            let _ = cr.fill();
            for side in [x + 2.0 + inset / 2.0, x + sw - 2.0 - inset / 2.0] {
                set(cr, (0.16, 0.16, 0.16));
                cr.arc(side, bottom - 4.0, 1.6, 0.0, 2.0 * PI);
                let _ = cr.fill();
            }

            // dissipador cinza com a textura de losangos
            let spreader = [
                (x, top),
                (x + sw, top),
                (x + sw, shoulder),
                (x + sw - inset, shoulder + inset),
                (x + sw - inset, bottom),
                (x + inset, bottom),
                (x + inset, shoulder + inset),
                (x, shoulder),
            ];
            let face = LinearGradient::new(x, top, x + sw * 0.3, bottom);
            face.add_color_stop_rgb(0.0, 0.38, 0.38, 0.39);
            face.add_color_stop_rgb(1.0, 0.25, 0.25, 0.26);
            let _ = cr.set_source(&face);
            polygon(cr, &spreader);
            let _ = cr.fill_preserve();
            cr.save().ok();
            cr.clip();
            let cell = sw / 11.0;
            let rise = cell * 0.42;
            cr.set_line_width(0.8);
            cr.set_source_rgba(0.0, 0.0, 0.0, 0.38);
            let mut k = -spreader_h / rise * cell;
            while k < sw + cell {
                cr.move_to(x + k, bottom);
                cr.line_to(x + k + spreader_h / rise * cell, top);
                cr.move_to(x + k, top);
                cr.line_to(x + k + spreader_h / rise * cell, bottom);
                k += cell;
            }
            let _ = cr.stroke();
            cr.restore().ok();
            set(cr, (0.46, 0.46, 0.47));
            cr.set_line_width(1.0);
            cr.move_to(x + 1.0, top + 0.5);
            cr.line_to(x + sw - 1.0, top + 0.5);
            let _ = cr.stroke();

            // contatos dourados, com o chanfro um pouco depois do meio
            let (cy, notch) = (bottom + pcb_h, x + sw * 0.54);
            set(cr, (0.05, 0.05, 0.05));
            cr.rectangle(x + 2.0, cy, notch - 2.5 - x - 2.0, contact_h);
            cr.rectangle(notch + 2.5, cy, x + sw - 2.0 - notch - 2.5, contact_h);
            let _ = cr.fill();
            set(cr, (0.80, 0.63, 0.26));
            let mut cx = x + sw * 0.035;
            while cx < x + sw * 0.965 {
                if (cx + 0.8 - notch).abs() > 4.0 {
                    cr.rectangle(cx, cy + 1.0, 1.6, contact_h - 1.0);
                }
                cx += 2.6;
            }
            let _ = cr.fill();

            let label = name.to_uppercase();
            self.draw_zone(cr, name, &label, (x, y, sw, bar_h), t, Side::Above);
            y += stick_h + gap;
        }
    }
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

const PCB: Color = (0.075, 0.078, 0.082);
const PART: Color = (0.035, 0.035, 0.038);
const EDGE: Color = (0.27, 0.27, 0.28);
const LEADER: Color = (0.42, 0.42, 0.42);

fn set(cr: &Context, (r, g, b): Color) {
    cr.set_source_rgb(r, g, b);
}

fn average(colors: &[Color]) -> Color {
    let n = colors.len().max(1) as f64;
    let sum = colors
        .iter()
        .fold((0.0, 0.0, 0.0), |a, c| (a.0 + c.0, a.1 + c.1, a.2 + c.2));
    (sum.0 / n, sum.1 / n, sum.2 / n)
}

fn rounded(cr: &Context, (x, y, w, h): Rect, radius: f64) {
    let radius = radius.min(w / 2.0).min(h / 2.0);
    cr.new_sub_path();
    cr.arc(x + w - radius, y + radius, radius, -PI / 2.0, 0.0);
    cr.arc(x + w - radius, y + h - radius, radius, 0.0, PI / 2.0);
    cr.arc(x + radius, y + h - radius, radius, PI / 2.0, PI);
    cr.arc(x + radius, y + radius, radius, PI, 1.5 * PI);
    cr.close_path();
}

fn polygon(cr: &Context, points: &[(f64, f64)]) {
    for (i, (x, y)) in points.iter().enumerate() {
        if i == 0 {
            cr.move_to(*x, *y);
        } else {
            cr.line_to(*x, *y);
        }
    }
    cr.close_path();
}

/// Silhueta da barra de luz do pente: cantos de cima chanfrados e um
/// rebaixo trapezoidal no meio, como na Loki W2.
fn light_bar(cr: &Context, (x, y, w, h): Rect) {
    let (chamfer, dip) = (h * 0.6, h * 0.22);
    polygon(
        cr,
        &[
            (x, y + h),
            (x + chamfer, y),
            (x + w * 0.385, y),
            (x + w * 0.405, y + dip),
            (x + w * 0.60, y + dip),
            (x + w * 0.62, y),
            (x + w - chamfer, y),
            (x + w, y + h),
        ],
    );
}

/// Peça sólida com contorno discreto.
fn block(cr: &Context, rect: Rect, color: Color) {
    set(cr, color);
    cr.rectangle(rect.0, rect.1, rect.2, rect.3);
    let _ = cr.fill_preserve();
    set(cr, EDGE);
    cr.set_line_width(0.6);
    let _ = cr.stroke();
}

/// Gradiente de metal escovado como fonte de cor.
fn metal(cr: &Context, (x, y, w, h): Rect) {
    let gradient = LinearGradient::new(x, y, x + w, y + h);
    gradient.add_color_stop_rgb(0.0, 0.33, 0.33, 0.34);
    gradient.add_color_stop_rgb(1.0, 0.14, 0.14, 0.15);
    let _ = cr.set_source(&gradient);
}

/// Dissipador de metal com aletas (verticais ou horizontais).
fn heatsink(cr: &Context, rect: Rect, fins: usize, vertical: bool) {
    let (x, y, w, h) = rect;
    metal(cr, rect);
    rounded(cr, rect, 1.5);
    let _ = cr.fill();
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.45);
    cr.set_line_width(1.0);
    for i in 1..=fins {
        let k = i as f64 / (fins + 1) as f64;
        if vertical {
            let fx = (x + w * k).round() + 0.5;
            cr.move_to(fx, y + 2.0);
            cr.line_to(fx, y + h - 2.0);
        } else {
            let fy = (y + h * k).round() + 0.5;
            cr.move_to(x + 2.0, fy);
            cr.line_to(x + w - 2.0, fy);
        }
    }
    let _ = cr.stroke();
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.14);
    cr.rectangle(x + 1.0, y, w - 2.0, 1.0);
    let _ = cr.fill();
}

/// Furos de pino espalhados sobre um conector.
fn pin_grid(cr: &Context, (x, y, w, h): Rect, color: Color) {
    let step = 2.6;
    let cols = ((w - 1.0) / step).floor().max(1.0);
    let rows = ((h - 1.0) / step).floor().max(1.0);
    let (ox, oy) = (x + (w - cols * step) / 2.0, y + (h - rows * step) / 2.0);
    set(cr, color);
    for i in 0..cols as usize {
        for j in 0..rows as usize {
            cr.rectangle(
                ox + i as f64 * step + 0.6,
                oy + j as f64 * step + 0.6,
                1.4,
                1.4,
            );
        }
    }
    let _ = cr.fill();
}

/// Conector de pinos (alimentação, ventoinha, painel frontal).
fn pins(cr: &Context, rect: Rect) {
    block(cr, rect, (0.13, 0.13, 0.13));
    pin_grid(cr, rect, (0.32, 0.32, 0.32));
}

/// Slot PCIe com o chanfro de chave; o x16 tem reforço metálico.
fn slot(cr: &Context, rect: Rect, armored: bool) {
    let (x, y, w, h) = rect;
    block(cr, rect, (0.11, 0.11, 0.11));
    set(cr, PART);
    cr.rectangle(x + 2.0, y + h / 2.0 - 1.0, w - 4.0, 2.0);
    let _ = cr.fill();
    set(cr, PCB);
    cr.rectangle(x + w * 0.13, y, 2.0, h);
    let _ = cr.fill();
    if armored {
        cr.set_source_rgb(0.62, 0.62, 0.64);
        cr.set_line_width(1.2);
        rounded(cr, (x - 1.0, y - 1.0, w + 2.0, h + 2.0), 1.0);
        let _ = cr.stroke();
        block(cr, (x + w, y - 1.0, 4.0, h + 2.0), (0.30, 0.30, 0.30));
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
