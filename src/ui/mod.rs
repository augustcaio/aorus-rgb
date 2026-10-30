//! Janela principal, no arranjo do RGB Fusion 2.0:
//! - início: dispositivos, efeitos e cor; muda tudo junto (sincronizado);
//! - dispositivo: desenho com as zonas; muda as zonas selecionadas.

mod art;
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

// grade: margens de 12, colunas separadas por 18 (múltiplos de 6, como o libadwaita)
const MARGIN: i32 = 12;
const COLUMN_GAP: i32 = 18;
const LEFT_WIDTH: i32 = 358;
const RAIL_SIZE: i32 = 24;
const DIAGRAM_WIDTH: i32 = LEFT_WIDTH - 40 - MARGIN;
const DIAGRAM_HEIGHT: i32 = 300;
// tela inicial: quadros de tamanho fixo; a coluna cresce com os dispositivos
const TILE_SIZE: i32 = 104;
const TILES_PER_ROW: usize = 3;
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
    // cada tela tem o próprio tamanho; a janela acompanha a tela visível
    let stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .hhomogeneous(false)
        .vhomogeneous(false)
        .interpolate_size(true)
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
        .resizable(false)
        .content(&toasts)
        .build();
    window.add_css_class("aorus");

    let rail: Vec<(Device, gtk::Button)> = Device::ALL
        .into_iter()
        .map(|device| {
            let icon = icons::area(RAIL_SIZE, move |cr, size| {
                icons::device(cr, device, size, theme::LINE)
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

    /// Título da coluna e conteúdo, alinhados no topo e à esquerda.
    fn section(title: &str, child: &impl IsA<gtk::Widget>) -> gtk::Box {
        let label = gtk::Label::new(Some(title));
        label.add_css_class("caption-heading");
        label.add_css_class("dim-label");
        label.set_xalign(0.0);
        let section = gtk::Box::new(gtk::Orientation::Vertical, 6);
        section.set_valign(gtk::Align::Start);
        section.append(&label);
        section.append(child);
        section
    }

    /// Três colunas (dispositivo/zonas, efeito, cor) e o rodapé, na mesma grade.
    fn page(left: &gtk::Box, controls: &Controls, footer: &gtk::Box) -> gtk::Box {
        let columns = gtk::Box::builder()
            .spacing(COLUMN_GAP)
            .vexpand(true)
            .valign(gtk::Align::Start)
            .build();
        columns.append(left);
        columns.append(&Self::section("Efeito", &controls.effects));
        columns.append(&Self::section("Cor", &controls.panel));
        let page = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(MARGIN)
            .margin_top(MARGIN)
            .margin_bottom(MARGIN)
            .margin_start(MARGIN)
            .margin_end(MARGIN)
            .build();
        page.append(&columns);
        page.append(footer);
        page
    }

    /// Rodapé: dica à esquerda e, se houver, ações à direita.
    fn footer(hint: &str, actions: Option<&gtk::Box>) -> gtk::Box {
        let label = gtk::Label::new(Some(hint));
        label.add_css_class("caption");
        label.add_css_class("dim-label");
        label.set_xalign(0.0);
        label.set_wrap(true);
        label.set_hexpand(true);
        // largura natural curta: a dica quebra linha em vez de alargar a janela
        label.set_max_width_chars(40);
        let footer = gtk::Box::new(gtk::Orientation::Horizontal, MARGIN);
        footer.append(&label);
        if let Some(actions) = actions {
            footer.append(actions);
        }
        footer
    }

    fn build_home(self: &Rc<Self>) {
        let tiles = gtk::Grid::builder()
            .column_spacing(MARGIN)
            .row_spacing(MARGIN)
            .column_homogeneous(true)
            .build();
        for (i, device) in Device::ALL.into_iter().enumerate() {
            let icon = icons::area(64, move |cr, size| {
                icons::device(cr, device, size, theme::LINE)
            });
            let frame = gtk::Button::builder()
                .child(&icon)
                .tooltip_text(format!("Ajustar {}", device.label()))
                .width_request(TILE_SIZE)
                .height_request(TILE_SIZE)
                .build();
            frame.add_css_class("device-tile");
            let name = gtk::Label::new(Some(device.label()));
            name.add_css_class("caption");
            name.add_css_class("tile-label");
            name.set_ellipsize(gtk::pango::EllipsizeMode::End);
            let tile = gtk::Box::new(gtk::Orientation::Vertical, 6);
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
            let (column, row) = (i % TILES_PER_ROW, i / TILES_PER_ROW);
            tiles.attach(&tile, column as i32, row as i32, 1, 1);
        }

        let left = Self::section("Dispositivos", &tiles);
        let footer = Self::footer(
            "Sincronizado: as mudanças valem para todos os dispositivos. Clique num dispositivo para ajustar cada zona.",
            None,
        );
        self.stack.add_named(
            &Self::page(&left, &self.home_controls, &footer),
            Some("home"),
        );
    }

    fn build_device(self: &Rc<Self>) {
        let rail = gtk::Box::new(gtk::Orientation::Vertical, 6);
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
        self.diagram.area.set_hexpand(false);
        self.diagram.area.set_vexpand(false);
        self.diagram
            .area
            .set_size_request(DIAGRAM_WIDTH, DIAGRAM_HEIGHT);
        let zones = gtk::Box::new(gtk::Orientation::Horizontal, MARGIN);
        zones.append(&rail);
        zones.append(&self.diagram.area);

        let apply = gtk::Button::with_label("Aplicar");
        apply.add_css_class("square-action");
        apply.set_tooltip_text(Some(
            "Fixa o estado atual como ponto de partida do Redefinir",
        ));
        let reset = gtk::Button::with_label("Redefinir");
        reset.add_css_class("square-action");
        reset.set_tooltip_text(Some(
            "Volta as zonas ao estado de quando esta tela foi aberta",
        ));
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        actions.append(&apply);
        actions.append(&reset);

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

        let left = Self::section("Zonas", &zones);
        left.set_width_request(LEFT_WIDTH);
        let footer = Self::footer(
            "Clique numa zona para selecioná-la; sem seleção, vale para o dispositivo inteiro.",
            Some(&actions),
        );
        self.stack.add_named(
            &Self::page(&left, &self.device_controls, &footer),
            Some("device"),
        );
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
