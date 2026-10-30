//! Desenhos de linha da placa e do pente de memória, os mesmos no diagrama,
//! na tela inicial e na barra lateral; só muda o nível de detalhe.
//!
//! Cada desenho tem proporção fixa e é encaixado com escala uniforme no
//! espaço disponível. A placa segue o leiaute do manual da B550M AORUS ELITE
//! (244 x 244 mm, painel traseiro à esquerda) e o pente, a silhueta da Asgard
//! Loki W2. Desenhos próprios, sem artes ou logotipos dos fabricantes.

use std::f64::consts::PI;

use gtk::cairo::Context;

pub type Rgb = (f64, f64, f64);
pub type Rect = (f64, f64, f64, f64);

/// Altura do pente dividida pela largura: barra de luz, dissipador, PCB e
/// contatos, medidos na foto do módulo.
pub const MEMORY_ASPECT: f64 = BAR + SPREADER + PCB + CONTACTS;
const BAR: f64 = 0.047;
const SPREADER: f64 = 0.23;
const PCB: f64 = 0.014;
const CONTACTS: f64 = 0.025;

/// Quanto se desenha: o diagrama mostra cada peça; os pictogramas, só o
/// que ainda se lê no tamanho deles.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Detail {
    Glyph,
    Icon,
    Full,
}

/// Maior retângulo com a proporção `aspect` (altura / largura) dentro de
/// `area`, centrado.
pub fn fit((x, y, w, h): Rect, aspect: f64) -> Rect {
    let fw = w.min(h / aspect);
    let fh = fw * aspect;
    (x + (w - fw) / 2.0, y + (h - fh) / 2.0, fw, fh)
}

/// Pincel: contorno principal na cor cheia e peças internas mais discretas
/// no diagrama, onde há muitas linhas.
struct Pen<'a> {
    cr: &'a Context,
    color: Rgb,
    parts: f64,
}

impl Pen<'_> {
    fn main(&self) {
        self.cr
            .set_source_rgb(self.color.0, self.color.1, self.color.2);
    }

    fn part(&self) {
        let (r, g, b) = self.color;
        self.cr.set_source_rgba(r, g, b, self.parts);
    }

    fn stroke(&self) {
        let _ = self.cr.stroke();
    }

    /// Retângulo alinhado aos pixels, para a linha de 1 px ficar nítida.
    fn rect(&self, (x, y, w, h): Rect) {
        let (x0, y0) = (x.round() + 0.5, y.round() + 0.5);
        let (x1, y1) = (
            ((x + w).round() - 0.5).max(x0),
            ((y + h).round() - 0.5).max(y0),
        );
        self.cr.rectangle(x0, y0, x1 - x0, y1 - y0);
    }

    fn hline(&self, x0: f64, x1: f64, y: f64) {
        let y = y.round() + 0.5;
        self.cr.move_to(x0.round(), y);
        self.cr.line_to(x1.round(), y);
    }

    fn vline(&self, x: f64, y0: f64, y1: f64) {
        let x = x.round() + 0.5;
        self.cr.move_to(x, y0.round());
        self.cr.line_to(x, y1.round());
    }

    fn circle(&self, cx: f64, cy: f64, r: f64) {
        self.cr.new_sub_path();
        self.cr.arc(cx, cy, r.max(1.0), 0.0, 2.0 * PI);
    }

    fn polygon(&self, points: &[(f64, f64)]) {
        polygon(self.cr, points);
    }
}

pub fn polygon(cr: &Context, points: &[(f64, f64)]) {
    for (i, (x, y)) in points.iter().enumerate() {
        if i == 0 {
            cr.move_to(*x, *y);
        } else {
            cr.line_to(*x, *y);
        }
    }
    cr.close_path();
}

fn rounded(cr: &Context, (x, y, w, h): Rect, radius: f64) {
    let radius = radius.min(w / 2.0).min(h / 2.0);
    cr.new_sub_path();
    cr.arc(x + w - radius, y + radius, radius, -PI / 2.0, 0.0);
    cr.arc(x + w - radius, y + h - radius, radius, 0.0, PI / 2.0);
    cr.arc(x + radius, y + h - radius, radius, PI / 2.0, PI);
    cr.arc(x + radius, y + radius, radius, PI, 1.5 * PI);
    cr.close_path();
}

/// Placa vista de cima em `area` (quadrada), em linhas de 1 px.
pub fn board(cr: &Context, area: Rect, color: Rgb, detail: Detail) {
    let (bx, by, s, _) = fit(area, 1.0);
    let pen = Pen {
        cr,
        color,
        parts: if detail == Detail::Full { 0.55 } else { 1.0 },
    };
    let r = |fx: f64, fy: f64, fw: f64, fh: f64| (bx + fx * s, by + fy * s, fw * s, fh * s);
    let p = |fx: f64, fy: f64| (bx + fx * s, by + fy * s);
    let x = |fx: f64| bx + fx * s;
    let y = |fy: f64| by + fy * s;
    cr.set_line_width(1.0);

    pen.main();
    rounded(
        cr,
        (bx + 0.5, by + 0.5, s - 1.0, s - 1.0),
        (s * 0.012).max(1.5),
    );
    pen.stroke();

    pen.part();
    if detail == Detail::Glyph {
        pen.rect(r(0.0, 0.0, 0.18, 0.46)); // painel traseiro
        pen.rect(r(0.37, 0.18, 0.25, 0.25)); // soquete
        for fx in [0.74, 0.86] {
            pen.vline(x(fx), y(0.08), y(0.56)); // memória
        }
        pen.hline(x(0.16), x(0.60), y(0.70)); // PCIe x16
        pen.rect(r(0.72, 0.72, 0.16, 0.14)); // chipset
        pen.stroke();
        return;
    }

    // painel traseiro: cobertura chanfrada e portas abaixo dela
    pen.polygon(&[
        p(0.0, 0.02),
        p(0.13, 0.02),
        p(0.13, 0.40),
        p(0.10, 0.47),
        p(0.0, 0.47),
    ]);
    for k in [0.0, 0.05] {
        cr.move_to(x(0.025), y(0.33 + k));
        cr.line_to(x(0.105), y(0.25 + k));
    }
    // dissipadores do VRM
    pen.rect(r(0.145, 0.10, 0.085, 0.37));
    pen.rect(r(0.27, 0.055, 0.30, 0.06));
    // soquete AM4 e processador
    pen.rect(r(0.38, 0.215, 0.185, 0.195));
    pen.rect(r(0.43, 0.27, 0.085, 0.085));
    // quatro slots DDR4
    for i in 0..4 {
        let fx = 0.717 + f64::from(i) * 0.032;
        if detail == Detail::Full {
            pen.rect(r(fx, 0.04, 0.022, 0.545));
        } else {
            pen.vline(x(fx + 0.011), y(0.04), y(0.585));
        }
    }
    // PCIe x16, x1 e x4; chipset
    if detail == Detail::Full {
        pen.rect(r(0.17, 0.68, 0.41, 0.025));
        pen.rect(r(0.17, 0.76, 0.10, 0.025));
        pen.rect(r(0.17, 0.835, 0.41, 0.025));
    } else {
        pen.hline(x(0.17), x(0.58), y(0.69));
        pen.hline(x(0.17), x(0.27), y(0.77));
        pen.hline(x(0.17), x(0.58), y(0.845));
    }
    rounded(cr, r(0.70, 0.70, 0.165, 0.13), 1.5);
    cr.move_to(x(0.72), y(0.81));
    cr.line_to(x(0.80), y(0.72));
    pen.rect(r(0.94, 0.19, 0.045, 0.185)); // ATX
    pen.stroke();
    if detail == Detail::Icon {
        return;
    }

    // daqui em diante só no diagrama
    pen.rect(r(0.0, 0.48, 0.085, 0.045)); // U32_2
    pen.rect(r(0.0, 0.535, 0.085, 0.05)); // áudio
    for k in 0..3 {
        pen.circle(x(0.027 + f64::from(k) * 0.027), y(0.56), 0.008 * s);
    }
    pen.stroke();
    // aletas dos dissipadores
    for i in 1..=6 {
        let fy = 0.10 + 0.37 * f64::from(i) / 7.0;
        pen.hline(x(0.16), x(0.215), y(fy));
    }
    for i in 1..=9 {
        let fx = 0.27 + 0.30 * f64::from(i) / 10.0;
        pen.vline(x(fx), y(0.065), y(0.105));
    }
    // módulo de retenção e alavanca do soquete
    for fy in [0.125, 0.43] {
        pen.rect(r(0.34, fy, 0.26, 0.06));
    }
    pen.rect(r(0.57, 0.20, 0.012, 0.22));
    pen.stroke();
    for fy in [0.155, 0.46] {
        for fx in [0.355, 0.585] {
            pen.circle(x(fx), y(fy), 0.008 * s);
        }
    }
    pen.stroke();
    // canal central de cada slot DDR4 e o reforço metálico do x16
    for i in 0..4 {
        let fx = 0.717 + f64::from(i) * 0.032;
        pen.vline(x(fx + 0.011), y(0.06), y(0.565));
    }
    for (fx, fw, fy) in [(0.17, 0.41, 0.68), (0.17, 0.10, 0.76), (0.17, 0.41, 0.835)] {
        pen.vline(x(fx + fw * 0.13), y(fy), y(fy + 0.025)); // chave
    }
    pen.rect((
        x(0.17) - 2.0,
        y(0.68) - 2.0,
        0.41 * s + 4.0,
        0.025 * s + 4.0,
    ));
    pen.stroke();
    // M.2: o de cima com dissipador, o de baixo exposto
    pen.rect(r(0.27, 0.597, 0.31, 0.045));
    pen.hline(x(0.29), x(0.56), y(0.62));
    pen.rect(r(0.585, 0.595, 0.02, 0.05));
    pen.rect(r(0.64, 0.745, 0.018, 0.04));
    pen.stroke();
    pen.circle(x(0.165), y(0.62), 0.007 * s);
    for fx in [0.31, 0.39, 0.455] {
        pen.circle(x(fx), y(0.765), 0.007 * s);
    }
    pen.stroke();
    // alimentação, ventoinhas, SATA, chips e headers da borda de baixo
    for rect in [
        r(0.18, 0.018, 0.07, 0.045), // ATX_12V
        r(0.61, 0.035, 0.05, 0.02),  // CPU_FAN
        r(0.18, 0.51, 0.02, 0.045),  // SYS_FAN1
        r(0.79, 0.885, 0.045, 0.02), // SYS_FAN2
        r(0.925, 0.70, 0.06, 0.075), // SATA
        r(0.925, 0.79, 0.06, 0.075),
        r(0.06, 0.685, 0.035, 0.035), // rede
        r(0.09, 0.815, 0.045, 0.045), // áudio
        r(0.44, 0.79, 0.05, 0.05),    // Super I/O
        r(0.37, 0.52, 0.03, 0.03),    // BIOS
        r(0.09, 0.885, 0.035, 0.025), // F_AUDIO
        r(0.31, 0.885, 0.05, 0.025),  // TPM
        r(0.37, 0.885, 0.05, 0.025),  // COM
        r(0.48, 0.885, 0.04, 0.025),  // F_USB1
        r(0.58, 0.885, 0.07, 0.025),  // F_U32
        r(0.69, 0.885, 0.08, 0.025),  // F_PANEL
    ] {
        pen.rect(rect);
    }
    pen.vline(x(0.9625), y(0.19), y(0.375)); // fileiras do ATX
    for fy in [0.70, 0.79] {
        pen.rect(r(0.935, fy + 0.02, 0.04, 0.035));
    }
    pen.stroke();
    // bateria e furos de fixação
    pen.circle(x(0.917), y(0.437), 0.032 * s);
    pen.circle(x(0.917), y(0.437), 0.02 * s);
    for (fx, fy) in [
        (0.68, 0.04),
        (0.905, 0.04),
        (0.035, 0.62),
        (0.905, 0.60),
        (0.035, 0.965),
        (0.905, 0.965),
    ] {
        pen.circle(x(fx), y(fy), 0.013 * s);
    }
    pen.stroke();
    // trilha que isola a seção de áudio
    cr.set_dash(&[2.0, 2.0], 0.0);
    pen.hline(x(0.01), x(0.17), y(0.645));
    pen.vline(x(0.17), y(0.645), y(0.95));
    pen.stroke();
    cr.set_dash(&[], 0.0);
}

/// Silhueta da barra de luz do pente: cantos de cima chanfrados e um
/// rebaixo trapezoidal no meio.
pub fn light_bar(cr: &Context, (x, y, w, h): Rect) {
    let (chamfer, dip) = (h * 0.6, h * 0.22);
    polygon(
        cr,
        &[
            (x, y + h),
            (x + chamfer, y),
            (x + w * 0.385, y),
            (x + w * 0.405, y + dip),
            (x + w * 0.60, y + dip),
            (x + w * 0.62, y),
            (x + w - chamfer, y),
            (x + w, y + h),
        ],
    );
}

/// Faixa da barra de luz dentro do retângulo do pente.
pub fn memory_bar((x, y, w, _): Rect) -> Rect {
    (x, y, w, w * BAR)
}

/// Pente de memória de lado em `area`, na proporção `MEMORY_ASPECT`.
pub fn memory(cr: &Context, area: Rect, color: Rgb, detail: Detail) {
    let (x, y, w, _) = fit(area, MEMORY_ASPECT);
    let pen = Pen {
        cr,
        color,
        parts: if detail == Detail::Full { 0.28 } else { 0.7 },
    };
    cr.set_line_width(1.0);
    let top = y + w * BAR;
    let bottom = top + w * SPREADER;
    let pins = bottom + w * PCB;
    let end = pins + w * CONTACTS;
    let inset = w * 0.022;
    let shoulder = top + w * SPREADER * 0.33;
    let notch = x + w * 0.54;
    let half = (w * 0.008).max(1.5);

    pen.main();
    light_bar(cr, (x + 0.5, y + 0.5, w - 1.0, w * BAR));
    let spreader = [
        (x + 0.5, top + 0.5),
        (x + w - 0.5, top + 0.5),
        (x + w - 0.5, shoulder),
        (x + w - inset, shoulder + inset),
        (x + w - inset, bottom - 0.5),
        (x + inset, bottom - 0.5),
        (x + inset, shoulder + inset),
        (x + 0.5, shoulder),
    ];
    pen.polygon(&spreader);
    // PCB: bordas laterais à mostra e a base com o chanfro
    let (left, right) = (x + w * 0.006, x + w * 0.994);
    cr.move_to(left + 0.5, shoulder + inset + 2.0);
    cr.line_to(left + 0.5, end - 0.5);
    cr.line_to(notch - half, end - 0.5);
    cr.line_to(notch - half, end - w * CONTACTS * 0.6);
    cr.line_to(notch + half, end - w * CONTACTS * 0.6);
    cr.line_to(notch + half, end - 0.5);
    cr.line_to(right - 0.5, end - 0.5);
    cr.line_to(right - 0.5, shoulder + inset + 2.0);
    pen.stroke();
    if detail == Detail::Glyph {
        return;
    }

    // textura de losangos do dissipador, recortada pela silhueta
    pen.part();
    let _ = cr.save();
    pen.polygon(&spreader);
    cr.clip();
    let height = w * SPREADER;
    let cell = if detail == Detail::Full {
        w / 11.0
    } else {
        w / 4.0
    };
    let run = height / 0.42;
    let mut k = -run;
    while k < w + cell {
        cr.move_to(x + k, bottom);
        cr.line_to(x + k + run, top);
        cr.move_to(x + k, top);
        cr.line_to(x + k + run, bottom);
        k += cell;
    }
    pen.stroke();
    let _ = cr.restore();

    // contatos dourados como traços curtos
    let step = if detail == Detail::Full {
        2.6
    } else {
        (w * 0.04).max(2.0)
    };
    let mut px = x + w * 0.035;
    while px < x + w * 0.965 {
        if (px - notch).abs() > half + 1.5 {
            pen.vline(px, pins + 1.0, end - 1.0);
        }
        px += step;
    }
    pen.stroke();
}
