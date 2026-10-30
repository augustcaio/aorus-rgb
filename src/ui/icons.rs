//! Pictogramas dos dispositivos e ícones dos efeitos, desenhados com cairo.
//!
//! Desenhos próprios, sem artes da Gigabyte: a placa e o pente de memória são
//! silhuetas simples; cada efeito tem um símbolo dentro de um círculo.

use std::f64::consts::PI;

use gtk::cairo::Context;
use gtk::prelude::*;

use crate::model::{Device, Effect};

pub type Rgb = (f64, f64, f64);

fn set(cr: &Context, (r, g, b): Rgb) {
    cr.set_source_rgb(r, g, b);
}

/// Pictograma sólido de um dispositivo, centrado num quadrado de lado `size`.
pub fn device(cr: &Context, device: Device, size: f64, color: Rgb, filled: bool) {
    set(cr, color);
    cr.set_line_width((size / 40.0).max(1.0));
    let fill_or_stroke = |cr: &Context| {
        if filled {
            let _ = cr.fill();
        } else {
            let _ = cr.stroke();
        }
    };
    match device {
        Device::Board => {
            let m = size * 0.16;
            let w = size - 2.0 * m;
            // contorno da placa
            cr.rectangle(m, m, w, w);
            fill_or_stroke(cr);
            // detalhes vazados (fundo escuro por cima)
            let detail = if filled { (0.04, 0.04, 0.04) } else { color };
            set(cr, detail);
            let u = w / 10.0;
            cr.rectangle(m + 1.2 * u, m + 1.2 * u, 3.0 * u, 3.0 * u); // soquete
            for i in 0..4 {
                cr.rectangle(
                    m + 5.6 * u + f64::from(i) * 0.9 * u,
                    m + 0.9 * u,
                    0.45 * u,
                    4.2 * u,
                ); // memória
            }
            for i in 0..2 {
                cr.rectangle(
                    m + 1.0 * u,
                    m + 6.0 * u + f64::from(i) * 1.6 * u,
                    6.5 * u,
                    0.55 * u,
                ); // PCIe
            }
            cr.rectangle(m + 0.5 * u, m + 0.5 * u, 0.5 * u, 2.0 * u); // painel traseiro
            if filled {
                let _ = cr.fill();
            } else {
                let _ = cr.stroke();
            }
        }
        Device::Memory => {
            let w = size * 0.80;
            let h = size * 0.30;
            let x = (size - w) / 2.0;
            let y = (size - h) / 2.0;
            cr.rectangle(x, y, w, h);
            fill_or_stroke(cr);
            let detail = if filled { (0.04, 0.04, 0.04) } else { color };
            set(cr, detail);
            for i in 0..6 {
                cr.rectangle(
                    x + w * 0.06 + f64::from(i) * w * 0.15,
                    y + h * 0.22,
                    w * 0.11,
                    h * 0.42,
                ); // chips
            }
            cr.rectangle(x + w * 0.47, y + h * 0.80, w * 0.05, h * 0.2); // chanfro do conector
            if filled {
                let _ = cr.fill();
            } else {
                let _ = cr.stroke();
            }
            set(cr, color);
            for i in 0..20 {
                cr.rectangle(
                    x + w * 0.04 + f64::from(i) * w * 0.047,
                    y + h,
                    w * 0.025,
                    h * 0.18,
                ); // contatos
            }
            let _ = cr.fill();
        }
    }
}

/// Ícone de efeito: um círculo com o símbolo do efeito, no lado `size`.
pub fn effect(cr: &Context, effect: Effect, size: f64, color: Rgb) {
    let c = size / 2.0;
    let r = size * 0.44;
    set(cr, color);
    cr.set_line_width(1.6);
    cr.set_line_cap(gtk::cairo::LineCap::Round);
    cr.set_line_join(gtk::cairo::LineJoin::Round);
    cr.arc(c, c, r, 0.0, 2.0 * PI);
    let _ = cr.stroke();
    let s = r * 0.55; // escala do símbolo
    match effect {
        Effect::Static => {
            cr.arc(c, c, s * 0.42, 0.0, 2.0 * PI);
            let _ = cr.stroke();
            for i in 0..8 {
                let a = f64::from(i) * PI / 4.0;
                cr.move_to(c + a.cos() * s * 0.65, c + a.sin() * s * 0.65);
                cr.line_to(c + a.cos() * s, c + a.sin() * s);
            }
            let _ = cr.stroke();
        }
        Effect::Off => {
            cr.arc(c, c, s * 0.8, -PI / 2.0 + 0.6, 3.0 * PI / 2.0 - 0.6);
            let _ = cr.stroke();
            cr.move_to(c, c - s * 0.95);
            cr.line_to(c, c - s * 0.15);
            let _ = cr.stroke();
        }
        Effect::Pulse => {
            cr.move_to(c - s, c);
            let steps = 24;
            for i in 0..=steps {
                let t = f64::from(i) / f64::from(steps);
                cr.line_to(c - s + 2.0 * s * t, c - (t * 2.0 * PI).sin() * s * 0.6);
            }
            let _ = cr.stroke();
        }
        Effect::Flash => {
            cr.move_to(c + s * 0.25, c - s);
            cr.line_to(c - s * 0.45, c + s * 0.1);
            cr.line_to(c + s * 0.05, c + s * 0.1);
            cr.line_to(c - s * 0.25, c + s);
            cr.line_to(c + s * 0.45, c - s * 0.15);
            cr.line_to(c - s * 0.05, c - s * 0.15);
            cr.close_path();
            let _ = cr.stroke();
        }
        Effect::Cycle => {
            cr.arc(c, c, s * 0.75, 0.3, 2.0 * PI - 0.6);
            let _ = cr.stroke();
            let a = 2.0 * PI - 0.6;
            let (x, y) = (c + a.cos() * s * 0.75, c + a.sin() * s * 0.75);
            cr.move_to(x - s * 0.35, y - s * 0.05);
            cr.line_to(x, y);
            cr.line_to(x + s * 0.05, y + s * 0.38);
            let _ = cr.stroke();
        }
        Effect::Random => {
            for (dx, dy) in [
                (-0.55, -0.45),
                (0.45, -0.6),
                (0.0, 0.05),
                (-0.5, 0.6),
                (0.6, 0.5),
            ] {
                cr.arc(c + dx * s, c + dy * s, s * 0.14, 0.0, 2.0 * PI);
                let _ = cr.fill();
            }
        }
        Effect::Rainbow => {
            for (i, k) in [1.0, 0.7, 0.4].into_iter().enumerate() {
                cr.arc(c, c + s * 0.45, s * k, PI, 2.0 * PI);
                if i == 2 {
                    cr.line_to(c + s * k, c + s * 0.45);
                }
                let _ = cr.stroke();
            }
        }
    }
}

/// `DrawingArea` de tamanho fixo que redesenha com a função dada.
pub fn area(size: i32, draw: impl Fn(&Context, f64) + 'static) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::builder()
        .content_width(size)
        .content_height(size)
        .build();
    area.set_draw_func(move |_, cr, w, h| draw(cr, f64::from(w.min(h))));
    area
}
