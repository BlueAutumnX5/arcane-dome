use macroquad::prelude::*;

fn window_conf() -> Conf {
    Conf {
        window_title: "Arcane Dome".to_owned(),
        window_width: 1280,
        window_height: 720,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    loop {
        clear_background(Color::from_rgba(27, 31, 42, 255));
        draw_text("Arcane Dome", 40.0, 80.0, 48.0, WHITE);
        draw_text(
            "Milestone 1 coming soon. Esc to quit.",
            40.0,
            120.0,
            24.0,
            GRAY,
        );
        if is_key_pressed(KeyCode::Escape) {
            break;
        }
        next_frame().await
    }
}
