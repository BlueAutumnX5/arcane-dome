use macroquad::prelude::*;

fn window_conf() -> Conf {
    Conf {
        window_title: "Arcane Dome".to_owned(),
        window_width: shot_size().0,
        window_height: shot_size().1,
        high_dpi: true,
        fullscreen: arcane_dome::app::start_fullscreen(),
        ..Default::default()
    }
}

/// Window size: 1280 x 720, or ARCANE_SHOT_SIZE=<w>x<h> for screenshot runs.
fn shot_size() -> (i32, i32) {
    std::env::var("ARCANE_SHOT_SIZE")
        .ok()
        .and_then(|v| {
            let (w, h) = v.split_once('x')?;
            Some((w.parse().ok()?, h.parse().ok()?))
        })
        .unwrap_or((1280, 720))
}

#[macroquad::main(window_conf)]
async fn main() {
    arcane_dome::app::run().await;
}
