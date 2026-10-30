//! Ícone do app no cabeçalho, em branco como o título. A forma vem do ícone
//! simbólico do pacote, lida aqui mesmo (só usa M, C e z absolutos), sem
//! carregador de SVG; assim o cabeçalho mostra sempre o ícone do projeto,
//! mesmo que o tema de ícones do usuário tenha outro com o mesmo nome.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::cairo::{self, Context, Operator};
use gtk::prelude::*;

const SVG: &str = include_str!(
    "../../data/icons/hicolor/symbolic/apps/io.github.augustcaio.AorusRGB.Gui-symbolic.svg"
);
/// Mesmo branco do título do cabeçalho.
const COLOR: (f64, f64, f64) = (0.949, 0.949, 0.949);

#[derive(Debug, PartialEq)]
enum Seg {
    Move(f64, f64),
    Curve([f64; 6]),
    Close,
}

/// Como cada caminho do SVG entra na máscara.
#[derive(Debug, PartialEq)]
enum Paint {
    Fill,
    /// preenche e contorna (o olho preenchido leva um traço que esconde a emenda)
    FillStroke(f64),
    /// caminho transparente: recorta o que estiver embaixo
    Clear,
}

struct Shape {
    segs: Vec<Seg>,
    paint: Paint,
}

fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let key = format!(" {name}=\"");
    let start = tag.find(&key)? + key.len();
    let end = start + tag[start..].find('"')?;
    Some(&tag[start..end])
}

fn parse_path(d: &str) -> Vec<Seg> {
    let mut segs = Vec::new();
    let mut numbers = Vec::new();
    let mut command = ' ';
    let mut token = String::new();
    let flush = |token: &mut String, numbers: &mut Vec<f64>| {
        if let Ok(n) = token.parse() {
            numbers.push(n);
        }
        token.clear();
    };
    let emit = |command: char, numbers: &mut Vec<f64>, segs: &mut Vec<Seg>| {
        match command {
            'M' => {
                for [x, y] in numbers.as_chunks::<2>().0 {
                    segs.push(Seg::Move(*x, *y));
                }
            }
            'C' => {
                for c in numbers.as_chunks::<6>().0 {
                    segs.push(Seg::Curve(*c));
                }
            }
            'z' | 'Z' => segs.push(Seg::Close),
            _ => {}
        }
        numbers.clear();
    };
    for ch in d.chars() {
        if ch.is_ascii_alphabetic() && ch != 'e' {
            flush(&mut token, &mut numbers);
            emit(command, &mut numbers, &mut segs);
            command = ch;
        } else if ch == '-' && !token.is_empty() && !token.ends_with('e') {
            flush(&mut token, &mut numbers);
            token.push(ch);
        } else if ch.is_ascii_digit() || ch == '.' || ch == '-' || ch == 'e' {
            token.push(ch);
        } else {
            flush(&mut token, &mut numbers);
        }
    }
    flush(&mut token, &mut numbers);
    emit(command, &mut numbers, &mut segs);
    segs
}

/// Caminhos do SVG simbólico, com a largura do viewBox.
fn shapes() -> (f64, Vec<Shape>) {
    let width = attr(SVG, "viewBox")
        .and_then(|v| v.split_whitespace().nth(2)?.parse().ok())
        .unwrap_or(650.0);
    let mut shapes = Vec::new();
    for chunk in SVG.split("<path").skip(1) {
        let Some(end) = chunk.find("</path>") else {
            continue;
        };
        let tag = &chunk[..end];
        let Some(d) = attr(tag, "d") else { continue };
        let paint = if attr(tag, "fill-opacity") == Some("0") {
            Paint::Clear
        } else {
            match (attr(tag, "stroke"), attr(tag, "stroke-width")) {
                (Some(stroke), Some(w)) if stroke != "none" => {
                    Paint::FillStroke(w.parse().unwrap_or(1.0))
                }
                _ => Paint::Fill,
            }
        };
        shapes.push(Shape {
            segs: parse_path(d),
            paint,
        });
    }
    (width, shapes)
}

/// Silhueta em branco opaco num quadrado de `px` pixels.
fn mask(px: i32) -> Option<cairo::ImageSurface> {
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, px, px).ok()?;
    let cr = Context::new(&surface).ok()?;
    let (width, shapes) = shapes();
    cr.scale(f64::from(px) / width, f64::from(px) / width);
    cr.set_source_rgb(1.0, 1.0, 1.0);
    for shape in &shapes {
        cr.new_path();
        for seg in &shape.segs {
            match *seg {
                Seg::Move(x, y) => cr.move_to(x, y),
                Seg::Curve([x1, y1, x2, y2, x, y]) => cr.curve_to(x1, y1, x2, y2, x, y),
                Seg::Close => cr.close_path(),
            }
        }
        match shape.paint {
            Paint::Fill => {
                cr.set_operator(Operator::Over);
                let _ = cr.fill();
            }
            Paint::FillStroke(w) => {
                cr.set_operator(Operator::Over);
                cr.set_line_width(w);
                let _ = cr.fill_preserve();
                let _ = cr.stroke();
            }
            Paint::Clear => {
                cr.set_operator(Operator::Clear);
                let _ = cr.fill();
            }
        }
    }
    drop(cr);
    Some(surface)
}

pub struct Logo {
    pub area: gtk::DrawingArea,
    cache: RefCell<Option<(i32, cairo::ImageSurface)>>,
}

impl Logo {
    pub fn new(size: i32) -> Rc<Self> {
        let area = gtk::DrawingArea::builder()
            .content_width(size)
            .content_height(size)
            .valign(gtk::Align::Center)
            .build();
        area.set_tooltip_text(Some("AORUS RGB"));
        let logo = Rc::new(Self {
            area: area.clone(),
            cache: RefCell::new(None),
        });
        let weak = Rc::downgrade(&logo);
        area.set_draw_func(move |area, cr, w, h| {
            if let Some(logo) = weak.upgrade() {
                logo.draw(cr, w.min(h), area.scale_factor());
            }
        });
        logo
    }

    fn draw(&self, cr: &Context, size: i32, scale: i32) {
        let px = size * scale.max(1);
        let cached = matches!(&*self.cache.borrow(), Some((p, _)) if *p == px);
        if !cached {
            let Some(surface) = mask(px) else { return };
            self.cache.replace(Some((px, surface)));
        }
        let cache = self.cache.borrow();
        let Some((_, surface)) = cache.as_ref() else {
            return;
        };
        cr.set_source_rgb(COLOR.0, COLOR.1, COLOR.2);
        cr.scale(1.0 / f64::from(scale.max(1)), 1.0 / f64::from(scale.max(1)));
        let _ = cr.mask_surface(surface, 0.0, 0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_absolute_moves_curves_and_close() {
        let segs = parse_path("M1,2 C3.5,4 5,-6 7e1,8 z");
        assert_eq!(
            segs,
            vec![
                Seg::Move(1.0, 2.0),
                Seg::Curve([3.5, 4.0, 5.0, -6.0, 70.0, 8.0]),
                Seg::Close
            ]
        );
    }

    #[test]
    fn symbolic_icon_has_the_silhouette_the_cutout_and_the_eye() {
        let (width, shapes) = shapes();
        assert_eq!(width, 650.0);
        let paints: Vec<&Paint> = shapes.iter().map(|s| &s.paint).collect();
        assert_eq!(
            paints,
            [&Paint::Fill, &Paint::Clear, &Paint::FillStroke(12.0)]
        );
        assert!(shapes.iter().all(|s| s.segs.len() > 3));
    }
}
