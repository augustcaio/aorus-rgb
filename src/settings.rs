//! Preferências da interface (não do hardware) em ~/.config/aorus-rgb/gui.ini.
//! A cena dos LEDs fica com o daemon; aqui só o que é da janela.

use std::path::PathBuf;

use gtk::glib;

const GROUP: &str = "interface";

pub struct Settings {
    /// `None` na simulação: lê os padrões e não grava nada.
    path: Option<PathBuf>,
    file: glib::KeyFile,
}

impl Settings {
    pub fn load(persist: bool) -> Self {
        let file = glib::KeyFile::new();
        let path = persist.then(|| glib::user_config_dir().join("aorus-rgb").join("gui.ini"));
        if let Some(path) = &path {
            // arquivo ausente ou inválido: fica com os padrões
            let _ = file.load_from_file(path, glib::KeyFileFlags::NONE);
        }
        Self { path, file }
    }

    /// Efeitos da tela inicial valem para todos os dispositivos (padrão: sim).
    pub fn sync(&self) -> bool {
        self.file.boolean(GROUP, "sync").unwrap_or(true)
    }

    pub fn set_sync(&self, sync: bool) {
        self.file.set_boolean(GROUP, "sync", sync);
        self.save();
    }

    /// Conectores da placa sem nada ligado (a placa não tem como detectar).
    pub fn unused(&self) -> Vec<String> {
        self.file
            .string_list("board", "unused")
            .map(|list| list.iter().map(|s| s.to_string()).collect())
            .unwrap_or_default()
    }

    pub fn set_unused(&self, names: &[String]) {
        // formato de lista do KeyFile ("a;b;"); o glib-rs não expõe set_string_list
        let list: String = names.iter().map(|n| format!("{n};")).collect();
        self.file.set_string("board", "unused", &list);
        self.save();
    }

    fn save(&self) {
        let Some(path) = &self.path else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = self.file.save_to_file(path) {
            eprintln!("aviso: não foi possível salvar {}: {e}", path.display());
        }
    }
}
