//! Janela principal, no arranjo do RGB Fusion 2.0:
//! - início: dispositivos, efeitos e cor; muda tudo junto (sincronizado);
//! - dispositivo: desenho com as zonas; muda as zonas selecionadas.

mod controls;
mod diagram;
mod icons;
mod theme;
mod wheel;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use gtk::glib;

use crate::Options;
use crate::backend::Backend;
use crate::model::{Device, Effect, Look, Zone, ZoneState};
use controls::Controls;
use diagram::Diagram;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Home,
    Board,
    Memory,
}

impl Page {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "home" => Some(Self::Home),
            "board" => Some(Self::Board),
            "memory" => Some(Self::Memory),
            _ => None,
        }
    }
}

pub fn load_css() {
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    let provider = gtk::CssProvider::new();
    provider.load_from_string(theme::CSS);
    // o tema é a identidade do app: com prioridade de aplicação, o libadwaita 1.10
    // ainda vencia nos cantos arredondados (testado), então usa a de usuário
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_USER,
    );
    // desenvolvimento: CSS extra para testar estilos sem recompilar
    if let Ok(path) = std::env::var("AORUS_RGB_EXTRA_CSS") {
        let extra = gtk::CssProvider::new();
        extra.load_from_path(path);
        gtk::style_context_add_provider_for_display(
            &display,
            &extra,
            gtk::STYLE_PROVIDER_PRIORITY_USER + 1,
        );
    }
}

struct App {
    window: adw::ApplicationWindow,
    backend: RefCell<Option<Backend>>,
    stack: gtk::Stack,
    title: gtk::Label,
    home_button: gtk::Button,
    toasts: adw::ToastOverlay,
    banner: adw::Banner,
    home_controls: Rc<Controls>,
    device_controls: Rc<Controls>,
    diagram: Rc<Diagram>,
    rail: Vec<(Device, gtk::Button)>,
    device: Cell<Device>,
    zones: RefCell<Vec<Zone>>,
    state: RefCell<HashMap<String, ZoneState>>,
    baseline: RefCell<HashMap<String, ZoneState>>,
    sending: Cell<bool>,
    pending: RefCell<Option<(Vec<String>, Look)>>,
}

pub fn build(app: &adw::Application, options: Options) {
    let home_controls = Controls::new();
    let device_controls = Controls::new();
    let diagram = Diagram::new();

    let title = gtk::Label::new(Some("AORUS RGB"));
    title.add_css_class("app-title");
    let home_button = gtk::Button::from_icon_name("go-home-symbolic");
    home_button.set_tooltip_text(Some("Início"));
    let header = adw::HeaderBar::builder().show_title(false).build();
    header.pack_start(&title);
    header.pack_end(&home_button);

    let banner = adw::Banner::new("");
    let stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .build();
    stack.add_css_class("main");
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.add_top_bar(&banner);
    toolbar.set_content(Some(&stack));
    let toasts = adw::ToastOverlay::new();
    toasts.set_child(Some(&toolbar));

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("AORUS RGB")
        .default_width(900)
        .default_height(540)
        .content(&toasts)
        .build();
    window.add_css_class("aorus");

    let rail: Vec<(Device, gtk::Button)> = Device::ALL
        .into_iter()
        .map(|device| {
            let icon = icons::area(32, move |cr, size| {
                icons::device(cr, device, size, (0.92, 0.92, 0.92), false)
            });
            let button = gtk::Button::builder()
                .child(&icon)
                .tooltip_text(device.label())
                .build();
            button.add_css_class("rail-tile");
            (device, button)
        })
        .collect();

    let app_state = Rc::new(App {
        window: window.clone(),
        backend: RefCell::new(None),
        stack,
        title,
        home_button,
        toasts,
        banner,
        home_controls,
        device_controls,
        diagram,
        rail,
        device: Cell::new(Device::Board),
        zones: RefCell::new(Vec::new()),
        state: RefCell::new(HashMap::new()),
        baseline: RefCell::new(HashMap::new()),
        sending: Cell::new(false),
        pending: RefCell::new(None),
    });
    app_state.build_home();
    app_state.build_device();
    app_state.connect();
    app_state.show(options.page);

    // a janela mantém o estado vivo; os callbacks guardam só referências fracas
    let keep_alive = app_state.clone();
    window.connect_destroy(move |_| {
        let _ = &keep_alive;
    });

    let mock = options.mock;
    let weak = Rc::downgrade(&app_state);
    glib::spawn_future_local(async move {
        let backend = if mock {
            Ok(Backend::mock())
        } else {
            Backend::dbus().await
        };
        let Some(app) = weak.upgrade() else { return };
        match backend {
            Ok(backend) => {
                let weak = Rc::downgrade(&app);
                backend.connect_changed(Rc::new(move || {
                    if let Some(app) = weak.upgrade() {
                        app.refresh();
                    }
                }));
                if backend.is_mock() {
                    app.window.set_title(Some("AORUS RGB (simulação)"));
                }
                app.backend.replace(Some(backend));
                app.refresh();
            }
            Err(message) => app.show_unavailable(&message),
        }
    });

    window.present();

    if let Some(path) = options.screenshot {
        let app = app.clone();
        let window = window.clone();
        glib::timeout_add_local_once(Duration::from_millis(1200), move || {
            match crate::screenshot::save(&window, &path) {
                Ok(()) => println!("captura salva em {path}"),
                Err(message) => eprintln!("erro na captura: {message}"),
            }
            app.quit();
        });
    }
}

impl App {
    // --- montagem -------------------------------------------------------------------

    fn build_home(self: &Rc<Self>) {
        let tiles = gtk::FlowBox::builder()
            .max_children_per_line(3)
            .min_children_per_line(3)
            .selection_mode(gtk::SelectionMode::None)
            .column_spacing(12)
            .row_spacing(12)
            .valign(gtk::Align::Start)
            .hexpand(true)
            .build();
        for device in Device::ALL {
            let icon = icons::area(72, move |cr, size| {
                icons::device(cr, device, size, theme::ORANGE, true)
            });
            let frame = gtk::Button::builder()
                .child(&icon)
                .tooltip_text(format!("Ajustar {}", device.label()))
                .build();
            frame.add_css_class("device-tile");
            let name = gtk::Label::new(Some(device.label()));
            name.add_css_class("tile-label");
            let tile = gtk::Box::new(gtk::Orientation::Vertical, 6);
            // quadro quadrado, com o pictograma no centro, como no RGB Fusion
            tile.set_halign(gtk::Align::Start);
            frame.set_halign(gtk::Align::Center);
            tile.append(&frame);
            tile.append(&name);
            let weak = Rc::downgrade(self);
            frame.connect_clicked(move |_| {
                if let Some(app) = weak.upgrade() {
                    app.show(match device {
                        Device::Board => Page::Board,
                        Device::Memory => Page::Memory,
                    });
                }
            });
            tiles.insert(&tile, -1);
        }

        let hint = gtk::Label::new(Some(
            "Sincronizado: efeito e cor valem para todos os dispositivos.\nClique num dispositivo para ajustar cada zona.",
        ));
        hint.add_css_class("diagram-hint");
        hint.set_xalign(0.0);
        let left = gtk::Box::new(gtk::Orientation::Vertical, 12);
        left.append(&tiles);
        left.append(&hint);

        let page = gtk::Box::builder()
            .spacing(18)
            .margin_top(18)
            .margin_bottom(18)
            .margin_start(20)
            .margin_end(20)
            .build();
        page.append(&left);
        page.append(&self.home_controls.effects);
        page.append(&self.home_controls.panel);
        self.stack.add_named(&page, Some("home"));
    }

    fn build_device(self: &Rc<Self>) {
        let rail = gtk::Box::new(gtk::Orientation::Vertical, 10);
        for (device, button) in &self.rail {
            let weak = Rc::downgrade(self);
            let device = *device;
            button.connect_clicked(move |_| {
                if let Some(app) = weak.upgrade() {
                    app.show(match device {
                        Device::Board => Page::Board,
                        Device::Memory => Page::Memory,
                    });
                }
            });
            rail.append(button);
        }

        let hint = gtk::Label::new(Some(
            "Clique numa zona para selecioná-la (clique de novo para tirar). Sem seleção, vale para o dispositivo inteiro.\nPosições dos conectores ilustrativas.",
        ));
        hint.add_css_class("diagram-hint");
        hint.set_wrap(true);
        let center = gtk::Box::new(gtk::Orientation::Vertical, 4);
        center.set_hexpand(true);
        center.append(&self.diagram.area);
        center.append(&hint);

        let apply = gtk::Button::with_label("APLICAR");
        apply.add_css_class("square-action");
        apply.set_tooltip_text(Some(
            "Fixa o estado atual como ponto de partida do REDEFINIR",
        ));
        let reset = gtk::Button::with_label("REDEFINIR");
        reset.add_css_class("square-action");
        reset.set_tooltip_text(Some(
            "Volta as zonas ao estado de quando esta tela foi aberta",
        ));
        let actions = gtk::Box::builder()
            .spacing(8)
            .halign(gtk::Align::End)
            .valign(gtk::Align::End)
            .vexpand(true)
            .build();
        actions.append(&apply);
        actions.append(&reset);
        let right = gtk::Box::new(gtk::Orientation::Vertical, 0);
        right.append(&self.device_controls.panel);
        right.append(&actions);

        let weak = Rc::downgrade(self);
        apply.connect_clicked(move |_| {
            if let Some(app) = weak.upgrade() {
                app.baseline.replace(app.state.borrow().clone());
                app.toast(
                    "Estado aplicado: o daemon já guarda a cena e a restaura no próximo login",
                );
            }
        });
        let weak = Rc::downgrade(self);
        reset.connect_clicked(move |_| {
            if let Some(app) = weak.upgrade() {
                app.reset();
            }
        });

        let page = gtk::Box::builder()
            .spacing(14)
            .margin_top(14)
            .margin_bottom(14)
            .margin_start(14)
            .margin_end(18)
            .build();
        page.append(&rail);
        page.append(&center);
        page.append(&self.device_controls.effects);
        page.append(&right);
        self.stack.add_named(&page, Some("device"));
    }

    fn connect(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.home_button.connect_clicked(move |_| {
            if let Some(app) = weak.upgrade() {
                app.show(Page::Home);
            }
        });
        let weak = Rc::downgrade(self);
        self.home_controls.connect_change(move |look| {
            if let Some(app) = weak.upgrade() {
                app.send(vec!["all".to_owned()], look);
            }
        });
        let weak = Rc::downgrade(self);
        self.device_controls.connect_change(move |look| {
            if let Some(app) = weak.upgrade() {
                let targets = app.targets(look.effect);
                app.send(targets, look);
            }
        });
        let weak = Rc::downgrade(self);
        self.diagram.connect_select(move || {
            if let Some(app) = weak.upgrade() {
                app.sync_controls();
            }
        });
    }

    // --- navegação --------------------------------------------------------------------

    fn show(&self, page: Page) {
        match page {
            Page::Home => {
                self.stack.set_visible_child_name("home");
                self.title.set_label("AORUS RGB");
                self.home_button.set_visible(false);
            }
            Page::Board | Page::Memory => {
                let device = if page == Page::Board {
                    Device::Board
                } else {
                    Device::Memory
                };
                self.device.set(device);
                self.diagram.set_device(device);
                self.title.set_label(&device.label().to_uppercase());
                self.home_button.set_visible(true);
                for (d, button) in &self.rail {
                    if *d == device {
                        button.add_css_class("selected");
                    } else {
                        button.remove_css_class("selected");
                    }
                }
                self.baseline.replace(self.state.borrow().clone());
                self.stack.set_visible_child_name("device");
            }
        }
        self.sync_controls();
    }

    fn on_device_page(&self) -> bool {
        self.stack.visible_child_name().as_deref() == Some("device")
    }

    /// Zonas do dispositivo atual que um comando deve atingir.
    fn device_zones(&self) -> Vec<Zone> {
        let device = self.device.get();
        let selected = self.diagram.selected();
        self.zones
            .borrow()
            .iter()
            .filter(|z| device.owns(z) && (selected.is_empty() || selected.contains(&z.name)))
            .cloned()
            .collect()
    }

    fn targets(&self, effect: Effect) -> Vec<String> {
        self.device_zones()
            .into_iter()
            .filter(|z| effect != Effect::Rainbow || z.kind.addressable())
            .map(|z| z.name)
            .collect()
    }

    /// Mostra nos controles o estado da primeira zona do alvo.
    fn sync_controls(&self) {
        let state = self.state.borrow();
        if self.on_device_page() {
            let zones = self.device_zones();
            let look = zones
                .iter()
                .find_map(|z| state.get(&z.name))
                .map(|s| Look::from_state(s, self.device_controls.look()))
                .unwrap_or_else(|| self.device_controls.look());
            self.device_controls.set_look(look);
            self.device_controls
                .set_rainbow_available(zones.iter().any(|z| z.kind.addressable()));
        } else {
            let zones = self.zones.borrow();
            let look = zones
                .iter()
                .filter(|z| z.kind.addressable())
                .find_map(|z| state.get(&z.name))
                .map(|s| Look::from_state(s, self.home_controls.look()))
                .unwrap_or_else(|| self.home_controls.look());
            self.home_controls.set_look(look);
        }
    }

    // --- backend ----------------------------------------------------------------------

    fn backend(&self) -> Option<Backend> {
        self.backend.borrow().clone()
    }

    fn refresh(self: &Rc<Self>) {
        let Some(backend) = self.backend() else {
            return;
        };
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let result =
                async { Ok::<_, String>((backend.zones().await?, backend.state().await?)) }.await;
            let Some(app) = weak.upgrade() else { return };
            match result {
                Ok((zones, state)) => {
                    app.banner.set_revealed(false);
                    let first_load = app.zones.borrow().is_empty();
                    app.diagram.set_zones(zones.clone());
                    app.diagram.set_state(state.clone());
                    app.zones.replace(zones);
                    app.state.replace(state);
                    if first_load {
                        app.baseline.replace(app.state.borrow().clone());
                    }
                    if !app.sending.get() {
                        app.sync_controls();
                    }
                }
                Err(message) => app.show_unavailable(&message),
            }
        });
    }

    /// Envia o visual às zonas, juntando mudanças rápidas (ex.: arrastar um slider).
    fn send(self: &Rc<Self>, targets: Vec<String>, look: Look) {
        if targets.is_empty() {
            return;
        }
        if self.sending.get() {
            self.pending.replace(Some((targets, look)));
            return;
        }
        let Some(backend) = self.backend() else {
            self.toast("Sem conexão com o daemon");
            return;
        };
        self.sending.set(true);
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let mut job = Some((targets, look));
            while let Some((targets, look)) = job.take() {
                for target in &targets {
                    if let Err(message) = backend.apply(target, &look).await {
                        if let Some(app) = weak.upgrade() {
                            app.toast(&message);
                        }
                        break;
                    }
                }
                job = weak.upgrade().and_then(|app| app.pending.take());
            }
            if let Some(app) = weak.upgrade() {
                app.sending.set(false);
            }
        });
    }

    fn reset(self: &Rc<Self>) {
        let device = self.device.get();
        let baseline = self.baseline.borrow().clone();
        let zones: Vec<Zone> = self
            .zones
            .borrow()
            .iter()
            .filter(|z| device.owns(z))
            .cloned()
            .collect();
        let Some(backend) = self.backend() else {
            return;
        };
        let fallback = self.device_controls.look();
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            for zone in zones {
                let Some(state) = baseline.get(&zone.name) else {
                    continue;
                };
                let look = Look::from_state(state, fallback);
                // a cor do estado já tem o brilho aplicado
                let look = if look.effect == Effect::Static {
                    Look {
                        brightness: 255,
                        ..look
                    }
                } else {
                    look
                };
                if let Err(message) = backend.apply(&zone.name, &look).await {
                    if let Some(app) = weak.upgrade() {
                        app.toast(&message);
                    }
                    return;
                }
            }
            if let Some(app) = weak.upgrade() {
                app.toast("Zonas redefinidas");
            }
        });
    }

    fn show_unavailable(&self, message: &str) {
        self.banner.set_title(&format!(
            "Daemon indisponível ({message}). Inicie com: systemctl --user start aorus-rgb"
        ));
        self.banner.set_revealed(true);
    }

    fn toast(&self, message: &str) {
        self.toasts.add_toast(adw::Toast::new(message));
    }
}
