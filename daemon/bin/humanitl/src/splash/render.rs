//! Ein Bild der drehenden Faust, als Zeilen für ein Terminal (HUM-496).
//!
//! Reine Funktionen ohne Ein- und Ausgabe, damit jedes Bild prüfbar ist. Der
//! Renderer ist klein und eigen, ohne GPU und ohne neue Abhängigkeit: Die
//! Silhouette aus [`super::fist`] wird zu einer Scheibe mit abgerundeten
//! Kanten in die Tiefe gezogen und je Bildpunkt mit einem Strahl abgetastet
//! (Ray-Marching gegen ein Dichtefeld). Die Normale an der Trefferstelle
//! kommt aus dem Gradienten desselben Felds, das Licht ist Lambert mit etwas
//! Glanz.
//!
//! Ein Terminal-Zeichen trägt zwei Bildpunkte übereinander: oben die
//! Vordergrundfarbe von `▀`, unten die Hintergrundfarbe. Weil eine Zelle etwa
//! doppelt so hoch wie breit ist, sind die Bildpunkte damit ungefähr
//! quadratisch. Ohne Farbe (`NO_COLOR`) steht je Zelle ein Zeichen einer
//! Helligkeitsleiter und keine einzige Steuerfolge.
//!
//! Die Datei steht für sich und nutzt aus dem Crate nur [`super::fist`]:
//! `examples/splash.rs` bindet beide über `#[path]` ein und zeigt die
//! Animation ohne Daemon.

use std::fmt::Write as _;

use super::fist;

/// Das Gelb der Marke (`docs/DESIGN.md`, Token `accent` im dunklen Thema).
pub const BRAND_YELLOW: (u8, u8, u8) = (0xE8, 0xB8, 0x4B);

/// Wie ein Bild gezeichnet wird.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Palette {
    /// 24-Bit-Farbe mit Halbblöcken.
    TrueColor,
    /// Ohne Farbe und ohne Steuerfolgen: eine Helligkeitsleiter aus ASCII.
    Mono,
}

/// Die Helligkeitsleiter ohne Farbe, dunkel nach hell. Ein Treffer bekommt
/// mindestens das zweite Zeichen, damit die Faust nie verschwindet.
const RAMP: &[u8] = b" .:-=+*#%@";

/// Wie tief die Faust ist, in Zellen der Silhouette.
const DEPTH: f32 = 14.0;

/// Breite der abgerundeten Kante zwischen Vorderseite und Flanke.
const BEVEL: f32 = 2.5;

/// Die feste Neigung um die waagrechte Achse, damit Ober- und Unterseiten der
/// Finger sichtbar werden.
const TILT: f32 = 0.28;

/// Schrittweite des Strahls in Zellen der Silhouette.
const STEP: f32 = 0.6;

/// Die Faust als Dichtefeld: die Bedeckung je Zelle, von 0 bis 1.
#[derive(Debug, Clone)]
pub struct Model {
    /// Bedeckung je Zelle, zeilenweise, oben zuerst.
    cover: Vec<f32>,
    /// Breite in Zellen.
    width: usize,
    /// Höhe in Zellen.
    height: usize,
}

impl Model {
    /// Die Faust des Logos.
    #[must_use]
    pub fn fist() -> Self {
        let cover = fist::ROWS
            .iter()
            .flat_map(|row| row.bytes())
            .map(|digit| f32::from(digit.saturating_sub(b'0')) / 9.0)
            .collect();
        Self {
            cover,
            width: fist::WIDTH,
            height: fist::HEIGHT,
        }
    }

    /// Die Bedeckung an einer Stelle der Ebene, bilinear zwischen den
    /// Zellmitten; außerhalb 0.
    fn coverage(&self, x: f32, y: f32) -> f32 {
        // Ursprung in der Mitte der Silhouette, y nach unten.
        #[allow(clippy::cast_precision_loss)]
        let (half_w, half_h) = (self.width as f32 / 2.0, self.height as f32 / 2.0);
        let gx = x + half_w - 0.5;
        let gy = y + half_h - 0.5;
        let x0 = gx.floor();
        let y0 = gy.floor();
        let fx = gx - x0;
        let fy = gy - y0;
        let at = |cx: f32, cy: f32| -> f32 {
            if cx < 0.0 || cy < 0.0 {
                return 0.0;
            }
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let (ix, iy) = (cx as usize, cy as usize);
            if ix >= self.width || iy >= self.height {
                return 0.0;
            }
            self.cover[iy * self.width + ix]
        };
        let top = at(x0, y0).mul_add(1.0 - fx, at(x0 + 1.0, y0) * fx);
        let bottom = at(x0, y0 + 1.0).mul_add(1.0 - fx, at(x0 + 1.0, y0 + 1.0) * fx);
        top.mul_add(1.0 - fy, bottom * fy)
    }

    /// Die Dichte im Raum des Modells: die Silhouette, begrenzt durch die
    /// Tiefe. Die Oberfläche liegt bei 0.5.
    fn density(&self, p: [f32; 3]) -> f32 {
        let slab = (0.5 + (DEPTH / 2.0 - p[2].abs()) / BEVEL).clamp(0.0, 1.0);
        self.coverage(p[0], p[1]).min(slab)
    }

    /// Der Radius der Kugel, in der das Modell bei jeder Drehung liegt.
    fn radius(&self) -> f32 {
        #[allow(clippy::cast_precision_loss)]
        let (w, h) = (self.width as f32, self.height as f32);
        (w.mul_add(w, h * h) + DEPTH * DEPTH).sqrt() / 2.0
    }

    /// Wie weit das Modell nach links und rechts reicht, über alle Winkel.
    fn reach_x(&self) -> f32 {
        #[allow(clippy::cast_precision_loss)]
        let w = self.width as f32;
        w.mul_add(w, DEPTH * DEPTH).sqrt() / 2.0
    }

    /// Wie weit das Modell nach oben und unten reicht, bei der festen
    /// Neigung.
    fn reach_y(&self) -> f32 {
        #[allow(clippy::cast_precision_loss)]
        let h = self.height as f32;
        (h / 2.0).mul_add(TILT.cos(), DEPTH / 2.0 * TILT.sin())
    }
}

/// Dreht einen Vektor um die senkrechte Achse.
fn yaw(p: [f32; 3], angle: f32) -> [f32; 3] {
    let (s, c) = angle.sin_cos();
    [
        p[0].mul_add(c, p[2] * s),
        p[1],
        (-p[0]).mul_add(s, p[2] * c),
    ]
}

/// Dreht einen Vektor um die waagrechte Achse.
fn pitch(p: [f32; 3], angle: f32) -> [f32; 3] {
    let (s, c) = angle.sin_cos();
    [p[0], p[1].mul_add(c, -p[2] * s), p[1].mul_add(s, p[2] * c)]
}

/// Ein Treffer: wie hell die Stelle ist, von 0 bis 1.
type Shade = Option<f32>;

/// Ein Bildpunkt: Der Strahl läuft vom Betrachter (`z` negativ) in die
/// Szene, im Raum des Modells.
fn shade(model: &Model, angle: f32, sx: f32, sy: f32) -> Shade {
    let radius = model.radius();
    if sx.abs() > model.reach_x() + 1.0 {
        return None;
    }
    // Welt nach Modell: erst die Neigung zurück, dann die Drehung.
    let to_model = |p: [f32; 3]| yaw(pitch(p, -TILT), -angle);
    let origin = to_model([sx, sy, -radius]);
    let dir = to_model([0.0, 0.0, 1.0]);
    let at = |t: f32| {
        [
            dir[0].mul_add(t, origin[0]),
            dir[1].mul_add(t, origin[1]),
            dir[2].mul_add(t, origin[2]),
        ]
    };
    let mut t = 0.0;
    let mut before = model.density(at(t));
    let end = 2.0 * radius;
    while t < end {
        let next = t + STEP;
        let now = model.density(at(next));
        if now >= 0.5 && before < 0.5 {
            // Die Kante zwischen den beiden Proben eingrenzen.
            let (mut lo, mut hi) = (t, next);
            for _ in 0..5 {
                let mid = f32::midpoint(lo, hi);
                if model.density(at(mid)) >= 0.5 {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            return Some(light(model, at(hi), angle));
        }
        before = now;
        t = next;
    }
    None
}

/// Die Länge eines Vektors auf 1 gebracht; ein Nullvektor bleibt `None`.
fn unit(vector: [f32; 3]) -> Option<[f32; 3]> {
    let length = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
    (length >= 1e-6).then(|| vector.map(|v| v / length))
}

/// Das Skalarprodukt zweier Vektoren.
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[2].mul_add(b[2], a[0].mul_add(b[0], a[1] * b[1]))
}

/// Die Helligkeit einer Stelle der Oberfläche.
fn light(model: &Model, point: [f32; 3], angle: f32) -> f32 {
    const EPS: f32 = 0.8;
    let density =
        |dx: f32, dy: f32, dz: f32| model.density([point[0] + dx, point[1] + dy, point[2] + dz]);
    let gradient = [
        density(EPS, 0.0, 0.0) - density(-EPS, 0.0, 0.0),
        density(0.0, EPS, 0.0) - density(0.0, -EPS, 0.0),
        density(0.0, 0.0, EPS) - density(0.0, 0.0, -EPS),
    ];
    // Die Normale zeigt aus dem Körper hinaus, also gegen den Gradienten;
    // zurück in die Welt erst die Drehung, dann die Neigung.
    let Some(normal) = unit(pitch(
        yaw([-gradient[0], -gradient[1], -gradient[2]], angle),
        TILT,
    )) else {
        return 0.6;
    };
    // Licht von links oben, zum Betrachter hin; der Blick geht nach +z.
    let to_light = unit([-0.45, -0.6, -0.66]).unwrap_or([0.0, 0.0, -1.0]);
    let diffuse = dot(normal, to_light).max(0.0);
    // Glanz: der halbe Weg zwischen Licht und Blick.
    let halfway = unit([to_light[0], to_light[1], to_light[2] - 1.0]).unwrap_or(to_light);
    let shine = dot(normal, halfway).max(0.0).powi(24);
    0.85f32.mul_add(diffuse, 0.22) + 0.5 * shine
}

/// Die Farbe zu einer Helligkeit: dunkles Ocker bis Gelb, im Glanz heller.
/// Auf 24 Stufen gerundet, damit Nachbarzellen dieselbe Steuerfolge teilen.
fn colour(level: f32) -> (u8, u8, u8) {
    let level = (level * 24.0).round() / 24.0;
    let base = level.min(1.0);
    let glow = (level - 1.0).max(0.0);
    let channel = |c: u8| {
        let value = f32::from(c).mul_add(base, (255.0 - f32::from(c)) * glow);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let out = value.clamp(0.0, 255.0).round() as u8;
        out
    };
    (
        channel(BRAND_YELLOW.0),
        channel(BRAND_YELLOW.1),
        channel(BRAND_YELLOW.2),
    )
}

/// Ein Bild der Faust beim Winkel `angle` (Bogenmaß), genau `rows` Zeilen
/// mit je genau `cols` sichtbaren Zeichen.
///
/// Mit [`Palette::TrueColor`] endet jede Zeile mit `ESC[0m`, damit keine
/// Farbe in die nächste Zeile oder in die Ausgabe danach läuft.
#[must_use]
pub fn frame(model: &Model, angle: f32, cols: usize, rows: usize, palette: Palette) -> Vec<String> {
    let width = cols;
    let height = rows * 2;
    if width == 0 || rows == 0 {
        return vec![String::new(); rows];
    }
    // Model-Einheiten je Bildpunkt: die Faust passt bei jedem Winkel hinein.
    #[allow(clippy::cast_precision_loss)]
    let scale = (2.0 * model.reach_x() / (width as f32 * 0.94))
        .max(2.0 * model.reach_y() / (height as f32 * 0.94));
    #[allow(clippy::cast_precision_loss)]
    let sample = |px: usize, py: usize| {
        let sx = (px as f32 + 0.5 - width as f32 / 2.0) * scale;
        let sy = (py as f32 + 0.5 - height as f32 / 2.0) * scale;
        shade(model, angle, sx, sy)
    };
    (0..rows)
        .map(|row| {
            let cells = (0..width).map(|col| (sample(col, 2 * row), sample(col, 2 * row + 1)));
            match palette {
                Palette::TrueColor => colour_line(cells),
                Palette::Mono => mono_line(cells),
            }
        })
        .collect()
}

/// Eine Zeile aus Halbblöcken in 24-Bit-Farbe.
fn colour_line(cells: impl Iterator<Item = (Shade, Shade)>) -> String {
    let mut line = String::new();
    let mut fg: Option<(u8, u8, u8)> = None;
    // `None` ist der Hintergrund des Terminals.
    let mut bg: Option<(u8, u8, u8)> = None;
    let set_fg = |line: &mut String, fg: &mut Option<(u8, u8, u8)>, c: (u8, u8, u8)| {
        if *fg != Some(c) {
            let _ = write!(line, "\x1b[38;2;{};{};{}m", c.0, c.1, c.2);
            *fg = Some(c);
        }
    };
    let set_bg = |line: &mut String, bg: &mut Option<(u8, u8, u8)>, c: Option<(u8, u8, u8)>| {
        if *bg != c {
            match c {
                Some(c) => {
                    let _ = write!(line, "\x1b[48;2;{};{};{}m", c.0, c.1, c.2);
                }
                None => line.push_str("\x1b[49m"),
            }
            *bg = c;
        }
    };
    for (top, bottom) in cells {
        match (top.map(colour), bottom.map(colour)) {
            (None, None) => {
                set_bg(&mut line, &mut bg, None);
                line.push(' ');
            }
            (Some(top), None) => {
                set_bg(&mut line, &mut bg, None);
                set_fg(&mut line, &mut fg, top);
                line.push('▀');
            }
            (None, Some(bottom)) => {
                set_bg(&mut line, &mut bg, None);
                set_fg(&mut line, &mut fg, bottom);
                line.push('▄');
            }
            (Some(top), Some(bottom)) => {
                set_bg(&mut line, &mut bg, Some(bottom));
                set_fg(&mut line, &mut fg, top);
                line.push('▀');
            }
        }
    }
    line.push_str("\x1b[0m");
    line
}

/// Eine Zeile ohne Farbe: je Zelle ein Zeichen der Helligkeitsleiter.
fn mono_line(cells: impl Iterator<Item = (Shade, Shade)>) -> String {
    cells
        .map(|cell| {
            let level = match cell {
                (None, None) => return ' ',
                (Some(a), Some(b)) => f32::midpoint(a, b),
                (Some(a), None) | (None, Some(a)) => a * 0.7,
            };
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                clippy::cast_precision_loss
            )]
            let index = ((level.clamp(0.0, 1.0) * (RAMP.len() - 1) as f32).round() as usize)
                .clamp(1, RAMP.len() - 1);
            char::from(RAMP[index])
        })
        .collect()
}

/// Wie viele Zeichen einer Zeile ein Terminal zeigt: ohne die Steuerfolgen
/// `ESC[ … m`.
#[cfg(test)]
#[must_use]
pub fn visible_width(line: &str) -> usize {
    let mut count = 0;
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            count += 1;
        }
    }
    count
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::{Model, Palette, frame, visible_width};

    /// Das Bild bei Winkel 0 ist nicht leer und passt genau in die Größe.
    #[test]
    fn a_frame_at_angle_zero_shows_the_fist_and_fits_the_size() {
        let model = Model::fist();
        for (cols, rows) in [(60, 30), (40, 20), (17, 9)] {
            let lines = frame(&model, 0.0, cols, rows, Palette::TrueColor);
            assert_eq!(lines.len(), rows, "{cols}x{rows}");
            for line in &lines {
                assert_eq!(visible_width(line), cols, "{cols}x{rows}: {line:?}");
                assert!(line.ends_with("\x1b[0m"), "the colour is reset per line");
            }
            let drawn: usize = lines
                .iter()
                .map(|line| line.matches(['▀', '▄']).count())
                .sum();
            // Die Faust füllt einen guten Teil der Fläche, nicht nur ein paar
            // Punkte.
            assert!(drawn * 6 > cols * rows, "{cols}x{rows}: only {drawn} cells");
        }
    }

    /// Die Faust dreht sich: Bilder bei verschiedenen Winkeln unterscheiden
    /// sich, und von der Seite ist sie schmaler als von vorn.
    #[test]
    fn the_fist_turns() {
        let model = Model::fist();
        let filled = |angle: f32| -> usize {
            frame(&model, angle, 60, 30, Palette::Mono)
                .iter()
                .map(|line| line.chars().filter(|c| *c != ' ').count())
                .sum()
        };
        let front = filled(0.0);
        let side = filled(std::f32::consts::FRAC_PI_2);
        assert!(side * 2 < front, "front {front}, side {side}");
        assert_ne!(
            frame(&model, 0.0, 60, 30, Palette::Mono),
            frame(&model, 0.4, 60, 30, Palette::Mono)
        );
    }

    /// Ohne Farbe steht keine einzige Steuerfolge im Bild, und jede Zeile hat
    /// genau die Breite.
    #[test]
    fn a_mono_frame_has_no_escape_codes() {
        let model = Model::fist();
        let lines = frame(&model, 0.7, 60, 30, Palette::Mono);
        assert_eq!(lines.len(), 30);
        for line in &lines {
            assert!(!line.contains('\x1b'), "{line:?}");
            assert_eq!(line.chars().count(), 60, "{line:?}");
        }
        assert!(lines.iter().any(|line| line.trim().len() > 10));
    }

    /// Eine Größe ohne Fläche liefert leere Zeilen und keine Panik.
    #[test]
    fn a_frame_without_area_is_empty() {
        let model = Model::fist();
        assert!(
            frame(&model, 0.0, 0, 3, Palette::TrueColor)
                .iter()
                .all(String::is_empty)
        );
        assert!(frame(&model, 0.0, 10, 0, Palette::Mono).is_empty());
    }
}
