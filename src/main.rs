use macroquad::prelude::*;

fn window_conf() -> Conf {
    Conf {
        window_title: "Arcane Dome".to_owned(),
        window_width: 1280,
        window_height: 720,
        high_dpi: true,
        fullscreen: arcane_dome::app::start_fullscreen(),
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    arcane_dome::app::run().await;
}
