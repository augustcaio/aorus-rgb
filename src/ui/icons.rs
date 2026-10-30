//! Pictogramas dos dispositivos e ícones dos efeitos, desenhados com cairo.
//!
//! Desenhos próprios, sem artes da Gigabyte: a placa e o pente de memória são
//! silhuetas tiradas do leiaute real; cada efeito tem um símbolo dentro de um
//! círculo.

use std::f64::consts::PI;

use gtk::cairo::Context;
use gtk::prelude::*;

use crate::model::{Device, Effect};

pub type Rgb = (f64, f64, f64);

fn set(cr: &Context, (r, g, b): Rgb) {
    cr.set_source_rgb(r, g, b);
}

/// Fundo escuro dos recortes, igual ao dos blocos da tela inicial.
const CUTOUT: Rgb = (0.043, 0.043, 0.043);

fn rect(cr: &Context, (x, y, w, h): (f64, f64, f64, f64)) {
    cr.rectangle(x, y, w, h);
}

/// Pictograma de um dispositivo, centrado num quadrado de lado `size`.
///
/// Preenchido (tela inicial), mostra as peças como recortes; em contorno
/// (barra lateral, 24 px), só o que ainda se lê nesse tamanho.
pub fn device(cr: &Context, device: Device, size: f64, color: Rgb, filled: bool) {
    match device {
        Device::Board => board(cr, size, color, filled),
        Device::Memory => memory(cr, size, color, filled),
    }
}

/// Placa vista de cima, nas proporções do leiaute do manual: painel traseiro
/// à esquerda, VRM, soquete, quatro slots de memória, PCIe e chipset.
fn board(cr: &Context, size: f64, color: Rgb, filled: bool) {
    let m = (size * 0.12).round();
    let w = size - 2.0 * m;
    let u = |fx: f64, fy: f64, fw: f64, fh: f64| (m + fx * w, m + fy * w, fw * w, fh * w);
    set(cr, color);
    if !filled {
        cr.set_line_width(1.0);
        cr.rectangle(m + 0.5, m + 0.5, w - 1.0, w - 1.0);
        let _ = cr.stroke();
        rect(cr, u(0.0, 0.0, 0.17, 0.45)); // painel traseiro
        rect(cr, u(0.38, 0.20, 0.24, 0.24)); // soquete
        for i in 0..3 {
            let x = (m + (0.70 + f64::from(i) * 0.09) * w).round();
            cr.rectangle(x, m + 0.08 * w, 1.0, 0.48 * w); // memória
        }
        rect(cr, u(0.14, 0.68, 0.46, 0.06)); // PCIe x16
        rect(cr, u(0.72, 0.70, 0.16, 0.14)); // chipset
        let _ = cr.fill();
        return;
    }
    cr.new_sub_path();
    let r = w * 0.04;
    cr.arc(m + w - r, m + r, r, -PI / 2.0, 0.0);
    cr.arc(m + w - r, m + w - r, r, 0.0, PI / 2.0);
    cr.arc(m + r, m + w - r, r, PI / 2.0, PI);
    cr.arc(m + r, m + r, r, PI, 1.5 * PI);
    cr.close_path();
    let _ = cr.fill();

    set(cr, CUTOUT);
    // cobertura do painel traseiro com o canto chanfrado
    cr.move_to(m + 0.03 * w, m + 0.04 * w);
    cr.line_to(m + 0.15 * w, m + 0.04 * w);
    cr.line_to(m + 0.15 * w, m + 0.40 * w);
    cr.line_to(m + 0.11 * w, m + 0.46 * w);
    cr.line_to(m + 0.03 * w, m + 0.46 * w);
    cr.close_path();
    rect(cr, u(0.19, 0.12, 0.07, 0.34)); // VRM à esquerda
    rect(cr, u(0.30, 0.05, 0.28, 0.06)); // VRM acima
    rect(cr, u(0.35, 0.15, 0.25, 0.28)); // soquete
    for i in 0..4 {
        rect(cr, u(0.70 + f64::from(i) * 0.06, 0.05, 0.035, 0.52)); // memória
    }
    rect(cr, u(0.92, 0.20, 0.045, 0.18)); // ATX
    rect(cr, u(0.14, 0.67, 0.46, 0.045)); // PCIe x16
    rect(cr, u(0.14, 0.755, 0.12, 0.035)); // PCIe x1
    rect(cr, u(0.14, 0.83, 0.46, 0.04)); // PCIe x4
    rect(cr, u(0.70, 0.69, 0.17, 0.14)); // chipset
    let _ = cr.fill();
    // processador dentro do soquete
    set(cr, color);
    rect(cr, u(0.40, 0.21, 0.15, 0.16));
    let _ = cr.fill();
}

/// Pente RGB de lado, na silhueta da Asgard Loki W2: barra de luz chanfrada
/// com rebaixo no meio, dissipador com losangos e os contatos com o chanfro.
fn memory(cr: &Context, size: f64, color: Rgb, filled: bool) {
    let w = (size * 0.86).round();
    let x = ((size - w) / 2.0).round();
    let bar = (w * 0.07).max(2.0).round();
    let body = (w * 0.23).max(5.0).round();
    let contacts = (w * 0.06).max(2.0).round();
    let gap = (w * 0.025).max(1.0).round();
    let total = bar + gap + body + gap + contacts;
    let y = ((size - total) / 2.0).round();
    let top = y + bar + gap;
    let pins = top + body + gap;
    set(cr, color);
    // barra de luz
    let (chamfer, dip) = (bar * 0.8, (bar * 0.35).max(1.0).round());
    cr.move_to(x, y + bar);
    cr.line_to(x + chamfer, y);
    cr.line_to(x + w * 0.385, y);
    cr.line_to(x + w * 0.405, y + dip);
    cr.line_to(x + w * 0.60, y + dip);
    cr.line_to(x + w * 0.62, y);
    cr.line_to(x + w - chamfer, y);
    cr.line_to(x + w, y + bar);
    cr.close_path();
    let _ = cr.fill();
    // dissipador, mais estreito embaixo
    let inset = (w * 0.03).max(1.0).round();
    let shoulder = top + body * 0.35;
    if filled {
        cr.move_to(x, top);
        cr.line_to(x + w, top);
        cr.line_to(x + w, shoulder);
        cr.line_to(x + w - inset, shoulder + inset);
        cr.line_to(x + w - inset, top + body);
        cr.line_to(x + inset, top + body);
        cr.line_to(x + inset, shoulder + inset);
        cr.line_to(x, shoulder);
        cr.close_path();
        let _ = cr.fill_preserve();
        let _ = cr.save();
        cr.clip();
        set(cr, CUTOUT);
        cr.set_line_width(1.0);
        let cell = w / 4.0;
        let run = body / 0.45;
        let mut k = -run;
        while k < w + cell {
            cr.move_to(x + k, top + body);
            cr.line_to(x + k + run, top);
            cr.move_to(x + k, top);
            cr.line_to(x + k + run, top + body);
            k += cell;
        }
        let _ = cr.stroke();
        let _ = cr.restore();
        set(cr, color);
    } else {
        cr.set_line_width(1.0);
        cr.rectangle(x + 0.5, top + 0.5, w - 1.0, body - 1.0);
        let _ = cr.stroke();
    }
    let notch = x + w * 0.54;
    let step = if filled { (w * 0.035).max(2.0) } else { 2.0 };
    let tooth = (step * 0.55).max(1.0);
    let mut px = x + w * 0.04;
    while px + tooth <= x + w * 0.96 {
        if (px + tooth / 2.0 - notch).abs() > step {
            cr.rectangle(px.round(), pins, tooth, contacts);
        }
        px += step;
    }
    let _ = cr.fill();
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
