#[macroquad::main(window_conf)]
async fn main() {
    arcane_dome::app::run().await;
}

fn window_conf() -> macroquad::window::Conf {
    arcane_dome::app::window_conf()
}
