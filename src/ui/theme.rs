//! Tema inspirado no arranjo do RGB Fusion 2.0: fundo quase preto, um único
//! destaque laranja e o resto em tons de cinza.

pub const ORANGE: (f64, f64, f64) = (0.910, 0.392, 0.106); // #e8641b
pub const GRAY: (f64, f64, f64) = (0.55, 0.55, 0.55);
pub const DIM: (f64, f64, f64) = (0.16, 0.16, 0.16);

pub const CSS: &str = r#"
@define-color accent_bg_color #e8641b;
@define-color accent_color #ff8a3d;
@define-color accent_fg_color #ffffff;

window.aorus,
window.aorus .main {
  background-color: #121212;
  background-image: linear-gradient(160deg, #222222 0%, #121212 55%, #0d0d0d 100%);
}

window.aorus headerbar {
  background: #141414;
  box-shadow: inset 0 -2px #e8641b;
  min-height: 42px;
}

.app-title {
  font-weight: 800;
  font-size: 15px;
  letter-spacing: 1px;
  color: #f2f2f2;
  margin-left: 8px;
}

window.aorus button.device-tile {
  background: #0b0b0b;
  border: 1px solid #5c5c5c;
  border-radius: 1px;
  padding: 6px;
}
window.aorus button.device-tile:hover { border-color: #e8641b; }
window.aorus button.device-tile.selected { border: 2px solid #e8641b; }
.tile-label { color: #9b9b9b; font-size: 11px; }

window.aorus button.rail-tile {
  background: #0b0b0b;
  border: 1px solid #4a4a4a;
  border-radius: 1px;
  padding: 3px;
}
window.aorus button.rail-tile.selected { border: 2px solid #e8641b; }

.effect-button {
  background: none;
  border: none;
  box-shadow: none;
  padding: 1px 3px;
  min-height: 0;
}
.effect-button:hover { background: alpha(#ffffff, 0.04); }
.effect-label { color: #8c8c8c; font-size: 9px; font-weight: 700; }
.effect-button.selected .effect-label { color: #e8641b; }
.effect-button:disabled .effect-label { color: #444444; }

window.aorus button.swatch {
  min-width: 18px;
  min-height: 11px;
  padding: 0;
  border-radius: 0;
  border: 1px solid #2c2c2c;
}
window.aorus button.swatch:hover { border-color: #e8641b; }

.rgb-label { color: #e8641b; font-weight: 800; font-size: 10px; }
.rgb-entry {
  min-width: 26px;
  min-height: 20px;
  padding: 0 4px;
  background: #3a3a3a;
  color: #dcdcdc;
  border-radius: 0;
  font-size: 10px;
}

.panel-label { color: #e6e6e6; font-weight: 700; font-size: 11px; }

window.aorus scale trough {
  background: #2c2c2c;
  min-height: 8px;
  border-radius: 0;
}
window.aorus scale highlight {
  background: #a9480f;
  border-radius: 0;
}
window.aorus scale slider {
  background: #e8641b;
  min-width: 16px;
  min-height: 16px;
  margin: -4px;
  box-shadow: none;
  border: none;
}

window.aorus button.square-action {
  background: #3b3b3b;
  color: #d8d8d8;
  border-radius: 0;
  font-weight: 700;
  min-width: 84px;
  min-height: 24px;
  font-size: 11px;
}
window.aorus button.square-action:hover { background: #4a4a4a; }

.diagram-hint { color: #6f6f6f; font-size: 10px; }
"#;
