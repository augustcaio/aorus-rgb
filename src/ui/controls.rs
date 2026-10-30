//! Coluna de efeitos e painel de cor (roda, cores rápidas, RGB/hex, sliders),
//! no arranjo do RGB Fusion 2.0.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::prelude::*;

use super::icons;
use super::theme::{GRAY, ORANGE};
use super::wheel::Wheel;
use crate::model::{Effect, Look, Rgb, SPEED_MAX, SPEED_MIN};

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
    brightness: gtk::Scale,
    look: Cell<Look>,
    updating: Cell<bool>,
    on_change: RefCell<Option<ChangeHandler>>,
}

fn label(text: &str, class: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class(class);
    label.set_xalign(0.0);
    label
}

fn scale(min: f64, max: f64) -> gtk::Scale {
    let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, min, max, 1.0);
    scale.set_draw_value(false);
    scale.set_hexpand(true);
    scale
}

impl Controls {
    pub fn new() -> Rc<Self> {
        let effects = gtk::Grid::builder()
            .row_spacing(2)
            .column_spacing(2)
            .valign(gtk::Align::Start)
            .build();
        let mut buttons = Vec::new();
        for (i, effect) in Effect::ALL.into_iter().enumerate() {
            let selected = Rc::new(Cell::new(false));
            let flag = selected.clone();
            let icon = icons::area(28, move |cr, size| {
                icons::effect(cr, effect, size, if flag.get() { ORANGE } else { GRAY });
            });
            let text = gtk::Label::new(Some(effect.label()));
            text.add_css_class("effect-label");
            text.set_wrap(true);
            text.set_justify(gtk::Justification::Center);
            text.set_max_width_chars(9);
            let content = gtk::Box::new(gtk::Orientation::Vertical, 1);
            content.append(&icon);
            content.append(&text);
            let button = gtk::Button::builder()
                .child(&content)
                .tooltip_text(effect.label())
                .build();
            button.add_css_class("effect-button");
            // ESTÁTICO e DESLIGADO lado a lado; os demais numa coluna, como no RGB Fusion
            let (column, row) = if i < 2 {
                (i as i32, 0)
            } else {
                (0, i as i32 - 1)
            };
            effects.attach(&button, column, row, 1, 1);
            buttons.push(EffectButton {
                effect,
                button,
                icon,
                selected,
            });
        }

        let panel = gtk::Box::new(gtk::Orientation::Vertical, 7);
        panel.set_width_request(200);
        let wheel = Wheel::new(150);
        wheel.area.set_halign(gtk::Align::Center);
        panel.append(&wheel.area);

        let swatches = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        swatches.set_halign(gtk::Align::Center);
        panel.append(&swatches);

        let rgb_row = gtk::Box::new(gtk::Orientation::Horizontal, 3);
        let entry = |chars: i32| {
            let entry = gtk::Entry::builder()
                .max_width_chars(chars)
                .width_chars(chars)
                .build();
            entry.add_css_class("rgb-entry");
            entry
        };
        let rgb = [entry(3), entry(3), entry(3)];
        let hex = entry(7);
        for (name, widget) in [("R", &rgb[0]), ("G", &rgb[1]), ("B", &rgb[2]), ("#", &hex)] {
            rgb_row.append(&label(name, "rgb-label"));
            rgb_row.append(widget);
        }
        panel.append(&rgb_row);

        let speed = scale(f64::from(SPEED_MIN), f64::from(SPEED_MAX));
        let brightness = scale(0.0, 255.0);
        panel.append(&label("Velocidade", "panel-label"));
        panel.append(&speed);
        panel.append(&label("Brilho", "panel-label"));
        panel.append(&brightness);

        let controls = Rc::new(Self {
            effects,
            panel,
            buttons,
            wheel,
            swatches,
            rgb,
            hex,
            speed,
            brightness,
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
            let area = icons::area(10, move |cr, _| {
                let (r, g, b) = color.to_f64();
                cr.set_source_rgb(r, g, b);
                let _ = cr.paint();
            });
            area.set_content_width(18);
            let button = gtk::Button::builder()
                .child(&area)
                .tooltip_text(format!("#{}", color.hex()))
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
        self.brightness.set_value(f64::from(look.brightness));
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
