//! Coluna de efeitos e painel de cor (roda, cores rápidas, RGB/hex, sliders),
//! no arranjo do RGB Fusion 2.0 e numa grade alinhada (espaçamentos de 6 px).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;

use super::icons;
use super::theme::{GRAY, ORANGE};
use super::wheel::Wheel;
use crate::model::{Effect, Look, Rgb, SPEED_MAX, SPEED_MIN};

pub const EFFECTS_WIDTH: i32 = 176;
pub const COLOR_WIDTH: i32 = 196;
const WHEEL_SIZE: i32 = 132;
const LABEL_WIDTH: i32 = 64;

/// Cores rápidas, como a fileira abaixo da roda do RGB Fusion.
const SWATCHES: [Rgb; 8] = [
    Rgb(255, 255, 255),
    Rgb(255, 0, 0),
    Rgb(255, 102, 0),
    Rgb(255, 210, 0),
    Rgb(0, 220, 60),
    Rgb(0, 200, 220),
    Rgb(0, 60, 255),
    Rgb(200, 0, 220),
];

type ChangeHandler = Box<dyn Fn(Look)>;

struct EffectButton {
    effect: Effect,
    button: gtk::Button,
    icon: gtk::DrawingArea,
    selected: Rc<Cell<bool>>,
}

pub struct Controls {
    pub effects: gtk::Grid,
    pub panel: gtk::Box,
    buttons: Vec<EffectButton>,
    wheel: Rc<Wheel>,
    swatches: gtk::Box,
    rgb: [gtk::Entry; 3],
    hex: gtk::Entry,
    speed: gtk::Scale,
    speed_value: gtk::Label,
    brightness: gtk::Scale,
    brightness_value: gtk::Label,
    look: Cell<Look>,
    updating: Cell<bool>,
    on_change: RefCell<Option<ChangeHandler>>,
}

fn caption(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("caption");
    label.set_xalign(0.0);
    label
}

fn entry(chars: i32) -> gtk::Entry {
    let entry = gtk::Entry::builder()
        .max_width_chars(chars)
        .width_chars(chars)
        .hexpand(true)
        .build();
    entry.add_css_class("rgb-entry");
    entry
}

/// Linha de slider alinhada em grade: rótulo | slider | valor.
fn slider_row(
    grid: &gtk::Grid,
    row: i32,
    name: &str,
    min: f64,
    max: f64,
) -> (gtk::Scale, gtk::Label) {
    let label = caption(name);
    label.set_width_request(LABEL_WIDTH);
    let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, min, max, 1.0);
    scale.set_draw_value(false);
    scale.set_hexpand(true);
    let value = caption("");
    value.add_css_class("numeric");
    value.set_xalign(1.0);
    value.set_width_chars(3);
    grid.attach(&label, 0, row, 1, 1);
    grid.attach(&scale, 1, row, 1, 1);
    grid.attach(&value, 2, row, 1, 1);
    (scale, value)
}

impl Controls {
    pub fn new() -> Rc<Self> {
        // efeitos: grade 2 x 4 de células iguais
        let effects = gtk::Grid::builder()
            .row_spacing(6)
            .column_spacing(6)
            .column_homogeneous(true)
            .valign(gtk::Align::Start)
            .width_request(EFFECTS_WIDTH)
            .build();
        let mut buttons = Vec::new();
        for (i, effect) in Effect::ALL.into_iter().enumerate() {
            let selected = Rc::new(Cell::new(false));
            let flag = selected.clone();
            let icon = icons::area(24, move |cr, size| {
                icons::effect(cr, effect, size, if flag.get() { ORANGE } else { GRAY });
            });
            icon.set_halign(gtk::Align::Center);
            let text = caption(effect.label());
            text.set_xalign(0.5);
            text.add_css_class("effect-label");
            let content = gtk::Box::new(gtk::Orientation::Vertical, 4);
            content.set_valign(gtk::Align::Center);
            content.append(&icon);
            content.append(&text);
            let button = gtk::Button::builder()
                .child(&content)
                .tooltip_text(effect.label())
                .height_request(54)
                .build();
            button.add_css_class("effect-button");
            let i = i as i32;
            effects.attach(&button, i % 2, i / 2, 1, 1);
            buttons.push(EffectButton {
                effect,
                button,
                icon,
                selected,
            });
        }

        // cor: roda, cores rápidas, RGB/hex e sliders, todos na largura do painel
        let panel = gtk::Box::new(gtk::Orientation::Vertical, 12);
        panel.set_width_request(COLOR_WIDTH);
        panel.set_valign(gtk::Align::Start);
        let wheel = Wheel::new(WHEEL_SIZE);
        wheel.area.set_halign(gtk::Align::Center);
        panel.append(&wheel.area);

        let swatches = gtk::Box::builder().spacing(4).homogeneous(true).build();
        panel.append(&swatches);

        let fields = gtk::Grid::builder()
            .row_spacing(6)
            .column_spacing(6)
            .build();
        let rgb = [entry(3), entry(3), entry(3)];
        for (i, (name, widget)) in [("R", &rgb[0]), ("G", &rgb[1]), ("B", &rgb[2])]
            .into_iter()
            .enumerate()
        {
            let label = caption(name);
            label.add_css_class("rgb-label");
            fields.attach(&label, i as i32 * 2, 0, 1, 1);
            fields.attach(widget, i as i32 * 2 + 1, 0, 1, 1);
        }
        let hex = entry(7);
        let hash = caption("#");
        hash.add_css_class("rgb-label");
        fields.attach(&hash, 0, 1, 1, 1);
        fields.attach(&hex, 1, 1, 5, 1);
        panel.append(&fields);

        let sliders = gtk::Grid::builder()
            .row_spacing(6)
            .column_spacing(6)
            .build();
        let (speed, speed_value) = slider_row(
            &sliders,
            0,
            "Velocidade",
            f64::from(SPEED_MIN),
            f64::from(SPEED_MAX),
        );
        let (brightness, brightness_value) = slider_row(&sliders, 1, "Brilho", 0.0, 255.0);
        panel.append(&sliders);

        let controls = Rc::new(Self {
            effects,
            panel,
            buttons,
            wheel,
            swatches,
            rgb,
            hex,
            speed,
            speed_value,
            brightness,
            brightness_value,
            look: Cell::new(Look::default()),
            updating: Cell::new(false),
            on_change: RefCell::new(None),
        });
        controls.connect_signals();
        controls.set_look(Look::default());
        controls
    }

    fn connect_signals(self: &Rc<Self>) {
        for button in &self.buttons {
            let weak = Rc::downgrade(self);
            let effect = button.effect;
            button.button.connect_clicked(move |_| {
                if let Some(c) = weak.upgrade() {
                    c.change(|look| look.effect = effect);
                }
            });
        }

        let weak = Rc::downgrade(self);
        self.wheel.connect_pick(move |color| {
            if let Some(c) = weak.upgrade() {
                c.change(|look| look.color = color);
            }
        });

        for color in SWATCHES {
            let area = gtk::DrawingArea::builder()
                .content_height(12)
                .hexpand(true)
                .build();
            area.set_draw_func(move |_, cr, _, _| {
                let (r, g, b) = color.to_f64();
                cr.set_source_rgb(r, g, b);
                let _ = cr.paint();
            });
            let button = gtk::Button::builder()
                .child(&area)
                .tooltip_text(format!("#{}", color.hex().to_uppercase()))
                .build();
            button.add_css_class("swatch");
            let weak = Rc::downgrade(self);
            button.connect_clicked(move |_| {
                if let Some(c) = weak.upgrade() {
                    c.change(|look| look.color = color);
                }
            });
            self.swatches.append(&button);
        }

        for entry in self.rgb.iter().chain([&self.hex]) {
            let weak = Rc::downgrade(self);
            entry.connect_activate(move |_| {
                if let Some(c) = weak.upgrade() {
                    c.color_from_entries();
                }
            });
            let focus = gtk::EventControllerFocus::new();
            let weak = Rc::downgrade(self);
            focus.connect_leave(move |_| {
                if let Some(c) = weak.upgrade() {
                    c.color_from_entries();
                }
            });
            entry.add_controller(focus);
        }

        let weak = Rc::downgrade(self);
        self.speed.connect_value_changed(move |scale| {
            if let Some(c) = weak.upgrade() {
                c.change(|look| look.speed = scale.value().round() as u8);
            }
        });
        let weak = Rc::downgrade(self);
        self.brightness.connect_value_changed(move |scale| {
            if let Some(c) = weak.upgrade() {
                c.change(|look| look.brightness = scale.value().round() as u8);
            }
        });
    }

    pub fn connect_change(&self, f: impl Fn(Look) + 'static) {
        self.on_change.replace(Some(Box::new(f)));
    }

    pub fn look(&self) -> Look {
        self.look.get()
    }

    /// Mostra um visual nos controles sem disparar `on_change`.
    pub fn set_look(&self, look: Look) {
        self.updating.set(true);
        self.look.set(look);
        for button in &self.buttons {
            let selected = button.effect == look.effect;
            button.selected.set(selected);
            if selected {
                button.button.add_css_class("selected");
            } else {
                button.button.remove_css_class("selected");
            }
            button.icon.queue_draw();
        }
        self.wheel.set_color(look.color);
        for (entry, value) in self
            .rgb
            .iter()
            .zip([look.color.0, look.color.1, look.color.2])
        {
            entry.set_text(&value.to_string());
        }
        self.hex.set_text(&look.color.hex().to_uppercase());
        self.speed.set_value(f64::from(look.speed));
        self.speed_value.set_label(&look.speed.to_string());
        self.brightness.set_value(f64::from(look.brightness));
        self.brightness_value
            .set_label(&look.brightness.to_string());
        let color_on = look.effect.uses_color();
        self.wheel.area.set_sensitive(color_on);
        self.swatches.set_sensitive(color_on);
        for entry in self.rgb.iter().chain([&self.hex]) {
            entry.set_sensitive(color_on);
        }
        self.speed.set_sensitive(look.effect.uses_speed());
        self.brightness.set_sensitive(look.effect != Effect::Off);
        self.updating.set(false);
    }

    /// Habilita o arco-íris só quando há zona endereçável no alvo.
    pub fn set_rainbow_available(&self, available: bool) {
        for button in &self.buttons {
            if button.effect == Effect::Rainbow {
                button.button.set_sensitive(available);
            }
        }
    }

    /// Escolhe um visual como se o usuário tivesse mexido nos controles
    /// (dispara `on_change`; usado pela gravação da demonstração).
    pub fn pick(&self, look: Look) {
        self.change(|current| *current = look);
    }

    fn change(&self, edit: impl FnOnce(&mut Look)) {
        if self.updating.get() {
            return;
        }
        let mut look = self.look.get();
        edit(&mut look);
        // escolher uma cor num efeito sem cor volta para o estático
        if look.color != self.look.get().color && !look.effect.uses_color() {
            look.effect = Effect::Static;
        }
        self.set_look(look);
        if let Some(f) = self.on_change.borrow().as_ref() {
            f(look);
        }
    }

    fn color_from_entries(&self) {
        let current = self.look.get().color;
        let hex = Rgb::parse_hex(&self.hex.text());
        let parse = |entry: &gtk::Entry, fallback: u8| {
            entry.text().trim().parse::<u8>().unwrap_or(fallback)
        };
        let typed = Rgb(
            parse(&self.rgb[0], current.0),
            parse(&self.rgb[1], current.1),
            parse(&self.rgb[2], current.2),
        );
        // o hex vale se foi ele que mudou; senão, os campos R, G e B
        let color = match hex {
            Some(hex) if hex != current => hex,
            _ => typed,
        };
        if color != current {
            self.change(|look| look.color = color);
        } else {
            self.set_look(self.look.get()); // desfaz texto inválido
        }
    }
}
