//! Roda de cores: matiz no ângulo, saturação no raio, como a do RGB Fusion.
//! O brilho fica no slider; a roda sempre mostra valor máximo.

use std::cell::{Cell, RefCell};
use std::f64::consts::PI;
use std::rc::Rc;

use gtk::cairo;
use gtk::prelude::*;

use crate::model::Rgb;

pub fn hsv_to_rgb(h: f64, s: f64, v: f64) -> Rgb {
    let h = h.rem_euclid(1.0) * 6.0;
    let i = h.floor();
    let f = h - i;
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    let (r, g, b) = match i as u8 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    let byte = |x: f64| (x * 255.0).round().clamp(0.0, 255.0) as u8;
    Rgb(byte(r), byte(g), byte(b))
}

/// (matiz, saturação) de uma cor, ignorando o brilho.
pub fn rgb_to_hs(color: Rgb) -> (f64, f64) {
    let (r, g, b) = color.to_f64();
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let s = if max > 0.0 { delta / max } else { 0.0 };
    if delta == 0.0 {
        return (0.0, s);
    }
    let h = if max == r {
        ((g - b) / delta).rem_euclid(6.0)
    } else if max == g {
        (b - r) / delta + 2.0
    } else {
        (r - g) / delta + 4.0
    };
    (h / 6.0, s)
}

type PickHandler = Box<dyn Fn(Rgb)>;

pub struct Wheel {
    pub area: gtk::DrawingArea,
    hs: Cell<(f64, f64)>,
    cache: RefCell<Option<(i32, cairo::ImageSurface)>>,
    on_pick: RefCell<Option<PickHandler>>,
}

impl Wheel {
    pub fn new(size: i32) -> Rc<Self> {
        let area = gtk::DrawingArea::builder()
            .content_width(size)
            .content_height(size)
            .build();
        area.set_cursor_from_name(Some("crosshair"));
        let wheel = Rc::new(Self {
            area: area.clone(),
            hs: Cell::new((0.07, 1.0)),
            cache: RefCell::new(None),
            on_pick: RefCell::new(None),
        });

        let weak = Rc::downgrade(&wheel);
        area.set_draw_func(move |_, cr, w, h| {
            if let Some(wheel) = weak.upgrade() {
                wheel.draw(cr, w.min(h));
            }
        });

        let drag = gtk::GestureDrag::new();
        let weak = Rc::downgrade(&wheel);
        drag.connect_drag_begin(move |_, x, y| {
            if let Some(wheel) = weak.upgrade() {
                wheel.pick(x, y);
            }
        });
        let weak = Rc::downgrade(&wheel);
        drag.connect_drag_update(move |gesture, dx, dy| {
            if let (Some(wheel), Some((x, y))) = (weak.upgrade(), gesture.start_point()) {
                wheel.pick(x + dx, y + dy);
            }
        });
        area.add_controller(drag);
        wheel
    }

    pub fn connect_pick(&self, f: impl Fn(Rgb) + 'static) {
        self.on_pick.replace(Some(Box::new(f)));
    }

    /// Posiciona o cursor numa cor, sem disparar `on_pick`.
    pub fn set_color(&self, color: Rgb) {
        let (h, s) = rgb_to_hs(color);
        // cor apagada (preto) não tem matiz: mantém a posição atual
        if color != Rgb::BLACK {
            self.hs.set((h, s));
        }
        self.area.queue_draw();
    }

    fn radius(&self) -> f64 {
        f64::from(self.area.width().min(self.area.height())) / 2.0 - 2.0
    }

    fn pick(&self, x: f64, y: f64) {
        let c = f64::from(self.area.width().min(self.area.height())) / 2.0;
        let (dx, dy) = (x - c, y - c);
        let s = ((dx * dx + dy * dy).sqrt() / self.radius()).min(1.0);
        let h = (dy.atan2(dx) / (2.0 * PI)).rem_euclid(1.0);
        self.hs.set((h, s));
        self.area.queue_draw();
        if let Some(f) = self.on_pick.borrow().as_ref() {
            f(hsv_to_rgb(h, s, 1.0));
        }
    }

    fn surface(&self, size: i32) -> Option<cairo::ImageSurface> {
        if let Some((cached, surface)) = self.cache.borrow().as_ref()
            && *cached == size
        {
            return Some(surface.clone());
        }
        let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, size, size).ok()?;
        let stride = surface.stride() as usize;
        let c = f64::from(size) / 2.0;
        let radius = c - 2.0;
        {
            let mut data = surface.data().ok()?;
            for y in 0..size {
                for x in 0..size {
                    let (dx, dy) = (f64::from(x) + 0.5 - c, f64::from(y) + 0.5 - c);
                    let dist = (dx * dx + dy * dy).sqrt();
                    // borda suavizada de ~1 px
                    let alpha = (radius + 0.5 - dist).clamp(0.0, 1.0);
                    if alpha <= 0.0 {
                        continue;
                    }
                    let h = (dy.atan2(dx) / (2.0 * PI)).rem_euclid(1.0);
                    let Rgb(r, g, b) = hsv_to_rgb(h, (dist / radius).min(1.0), 1.0);
                    let a = (alpha * 255.0) as u8;
                    let pre = |v: u8| (u16::from(v) * u16::from(a) / 255) as u8;
                    let i = y as usize * stride + x as usize * 4;
                    // ARGB32 em little-endian: B, G, R, A (pré-multiplicado)
                    data[i] = pre(b);
                    data[i + 1] = pre(g);
                    data[i + 2] = pre(r);
                    data[i + 3] = a;
                }
            }
        }
        self.cache.replace(Some((size, surface.clone())));
        Some(surface)
    }

    fn draw(&self, cr: &cairo::Context, size: i32) {
        // desativada (efeito sem cor): roda apagada e sem cursor
        let active = self.area.is_sensitive();
        if let Some(surface) = self.surface(size) {
            let _ = cr.set_source_surface(&surface, 0.0, 0.0);
            let _ = cr.paint_with_alpha(if active { 1.0 } else { 0.25 });
        }
        if !active {
            return;
        }
        let c = f64::from(size) / 2.0;
        let (h, s) = self.hs.get();
        let (x, y) = (
            c + (h * 2.0 * PI).cos() * s * self.radius(),
            c + (h * 2.0 * PI).sin() * s * self.radius(),
        );
        cr.set_line_width(2.0);
        cr.set_source_rgb(1.0, 1.0, 1.0);
        cr.arc(x, y, 6.0, 0.0, 2.0 * PI);
        let _ = cr.stroke();
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.6);
        cr.set_line_width(1.0);
        cr.arc(x, y, 7.5, 0.0, 2.0 * PI);
        let _ = cr.stroke();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsv_primaries() {
        assert_eq!(hsv_to_rgb(0.0, 1.0, 1.0), Rgb(255, 0, 0));
        assert_eq!(hsv_to_rgb(1.0 / 3.0, 1.0, 1.0), Rgb(0, 255, 0));
        assert_eq!(hsv_to_rgb(2.0 / 3.0, 1.0, 1.0), Rgb(0, 0, 255));
        assert_eq!(hsv_to_rgb(0.5, 0.0, 1.0), Rgb(255, 255, 255));
    }

    #[test]
    fn hue_and_saturation_roundtrip() {
        for color in [Rgb(255, 102, 0), Rgb(30, 224, 213), Rgb(140, 0, 255)] {
            let (h, s) = rgb_to_hs(color);
            let back = hsv_to_rgb(h, s, f64::from(color.0.max(color.1).max(color.2)) / 255.0);
            let close = |a: u8, b: u8| (i16::from(a) - i16::from(b)).abs() <= 1;
            assert!(
                close(back.0, color.0) && close(back.1, color.1) && close(back.2, color.2),
                "{color:?}"
            );
        }
    }
}
