//! Interface do aorus-rgb: fala com o daemon `io.github.augustcaio.AorusRGB`.
//!
//! Opções:
//!   --mock               hardware simulado, sem daemon (para validar a interface)
//!   --screenshot ARQ     salva a janela em PNG e fecha (validação automática)
//!   --page PÁGINA        página inicial: home, board ou memory

mod backend;
mod model;
mod screenshot;
mod ui;

use adw::prelude::*;
use gtk::glib;

const APP_ID: &str = "io.github.augustcaio.AorusRGB.Gui";

#[derive(Clone, Debug, Default)]
pub struct Options {
    pub mock: bool,
    pub screenshot: Option<String>,
    pub page: ui::Page,
}

fn parse_args() -> Result<Options, String> {
    let mut options = Options::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--mock" => options.mock = true,
            "--screenshot" => {
                options.screenshot = Some(args.next().ok_or("--screenshot precisa de um arquivo")?)
            }
            "--page" => {
                let page = args
                    .next()
                    .ok_or("--page precisa de home, board ou memory")?;
                options.page =
                    ui::Page::parse(&page).ok_or(format!("página desconhecida: {page}"))?;
            }
            "-h" | "--help" => {
                println!(
                    "uso: aorus-rgb-gui [--mock] [--screenshot ARQ.png] [--page home|board|memory]"
                );
                std::process::exit(0);
            }
            other => return Err(format!("opção desconhecida: {other}")),
        }
    }
    Ok(options)
}

fn main() -> glib::ExitCode {
    let options = match parse_args() {
        Ok(options) => options,
        Err(message) => {
            eprintln!("erro: {message}");
            return glib::ExitCode::FAILURE;
        }
    };
    // várias instâncias só para captura/simulação; o app normal é único
    let flags = if options.mock || options.screenshot.is_some() {
        gtk::gio::ApplicationFlags::NON_UNIQUE
    } else {
        gtk::gio::ApplicationFlags::empty()
    };
    let app = adw::Application::builder()
        .application_id(APP_ID)
        .flags(flags)
        .build();
    app.connect_startup(|_| {
        adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceDark);
        ui::load_css();
    });
    app.connect_activate(move |app| ui::build(app, options.clone()));
    // os argumentos já foram lidos acima; o GApplication não deve vê-los
    app.run_with_args::<&str>(&[])
}
