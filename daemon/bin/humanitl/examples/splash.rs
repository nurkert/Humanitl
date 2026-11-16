//! Die Ladeanimation von `humanitl go` ohne Daemon und ohne Sitzung
//! (HUM-496): zum Ansehen und zum Aufzeichnen.
//!
//! ```sh
//! cargo run -p humanitl --example splash               # 5 s live, Ctrl+C beendet
//! cargo run -p humanitl --example splash -- 10         # 10 s live
//! NO_COLOR=1 cargo run -p humanitl --example splash    # ohne Farbe
//! cargo run -p humanitl --example splash -- --dump DIR # 3 s als Textdateien
//! ```
//!
//! Gezeigt wird genau der Code, den `humanitl go` nutzt: `src/splash/` wird
//! über `#[path]` eingebunden, nicht nachgebaut.

#![allow(dead_code, clippy::print_stdout, clippy::print_stderr)]

use std::fmt::Write as _;
use std::path::Path;
use std::time::Duration;

/// Die Größe des Terminals, wie `humanitl` sie liest.
mod tty {
    /// Spalten und Zeilen des Terminals auf `stdin`, sonst 80 × 24.
    pub fn window_size() -> (u32, u32) {
        rustix::termios::tcgetwinsize(rustix::stdio::stdin()).map_or((80, 24), |size| {
            if size.ws_col == 0 || size.ws_row == 0 {
                (80, 24)
            } else {
                (u32::from(size.ws_col), u32::from(size.ws_row))
            }
        })
    }
}

#[path = "../src/splash/mod.rs"]
mod splash;

/// Bilder je Sekunde der Aufzeichnung.
const FPS: u32 = 30;

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let palette = splash::palette(std::env::var_os("NO_COLOR").as_deref());
    if let Some(at) = args.iter().position(|arg| arg == "--dump") {
        let dir = args.get(at + 1).map_or(".", String::as_str);
        return dump(Path::new(dir));
    }
    let seconds: u64 = args.first().and_then(|arg| arg.parse().ok()).unwrap_or(5);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        let Some(mut shown) = splash::Splash::start(palette) else {
            eprintln!("no thread for the animation");
            return;
        };
        tokio::select! {
            () = tokio::time::sleep(Duration::from_secs(seconds)) => {}
            _ = tokio::signal::ctrl_c() => {}
        }
        shown.stop();
    });
    Ok(())
}

/// Schreibt drei Sekunden der Animation als Text: jedes Bild in Farbe und
/// ohne, und ein Standbild von jedem.
fn dump(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let model = splash::render::Model::fist();
    let (cols, rows) = (80, 34);
    let mut colour = String::new();
    let mut mono = String::new();
    for index in 0..3 * FPS {
        let elapsed = Duration::from_secs(1) * index / FPS;
        let _ = write!(colour, "\x1b[H\x1b[2J");
        colour.push_str(&splash::screen(
            &model,
            elapsed,
            cols,
            rows,
            splash::Palette::TrueColor,
        ));
        colour.push('\n');
        let _ = writeln!(mono, "--- frame {index} at {} ms ---", elapsed.as_millis());
        for line in splash::render::frame(
            &model,
            splash::angle(elapsed),
            64,
            30,
            splash::Palette::Mono,
        ) {
            mono.push_str(line.trim_end());
            mono.push('\n');
        }
    }
    std::fs::write(dir.join("frames-color.ans"), colour)?;
    std::fs::write(dir.join("frames-mono.txt"), mono)?;
    let still = Duration::from_millis(350);
    let mut colour_still = splash::render::frame(
        &model,
        splash::angle(still),
        64,
        30,
        splash::Palette::TrueColor,
    )
    .join("\n");
    colour_still.push('\n');
    std::fs::write(dir.join("still-color.ans"), colour_still)?;
    let mono_still: Vec<String> =
        splash::render::frame(&model, splash::angle(still), 64, 30, splash::Palette::Mono)
            .iter()
            .map(|line| line.trim_end().to_owned())
            .collect();
    std::fs::write(dir.join("still-mono.txt"), mono_still.join("\n") + "\n")?;
    println!("wrote {} frames to {}", 3 * FPS, dir.display());
    Ok(())
}
