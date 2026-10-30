//! Pictogramas dos dispositivos e ícones dos efeitos, desenhados com cairo.
//!
//! Os dispositivos usam os desenhos de linha de `art`; cada efeito tem um
//! símbolo dentro de um círculo, no mesmo traço.

use std::f64::consts::PI;

use gtk::cairo::Context;
use gtk::prelude::*;

use super::art;
use crate::model::{Device, Effect};

pub type Rgb = (f64, f64, f64);

fn set(cr: &Context, (r, g, b): Rgb) {
    cr.set_source_rgb(r, g, b);
}

/// Pictograma de um dispositivo num quadrado de lado `size`: o mesmo desenho
/// de linhas do diagrama, com menos detalhe.
pub fn device(cr: &Context, device: Device, size: f64, color: Rgb) {
    let detail = if size >= 48.0 {
        art::Detail::Icon
    } else {
        art::Detail::Glyph
    };
    match device {
        Device::Board => {
            let m = (size * 0.12).round();
            art::board(cr, (m, m, size - 2.0 * m, size - 2.0 * m), color, detail);
        }
        Device::Memory => {
            let m = (size * 0.07).round();
            art::memory(cr, (m, m, size - 2.0 * m, size - 2.0 * m), color, detail);
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
