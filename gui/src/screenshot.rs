//! Salva a janela em PNG, para validar a interface sem intervenção.

use gtk::prelude::*;

pub fn save(widget: &impl IsA<gtk::Widget>, path: &str) -> Result<(), String> {
    let widget = widget.as_ref();
    let (width, height) = (widget.width(), widget.height());
    if width == 0 || height == 0 {
        return Err("a janela ainda não tem tamanho".into());
    }
    let paintable = gtk::WidgetPaintable::new(Some(widget));
    let snapshot = gtk::Snapshot::new();
    paintable.snapshot(&snapshot, f64::from(width), f64::from(height));
    let node = snapshot.to_node().ok_or("nada foi desenhado")?;
    let renderer = widget
        .native()
        .and_then(|n| n.renderer())
        .ok_or("sem renderizador")?;
    let texture = renderer.render_texture(&node, None);
    texture.save_to_png(path).map_err(|e| e.to_string())
}
