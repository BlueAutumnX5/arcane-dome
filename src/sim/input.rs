use macroquad::math::Vec2;

/// What one fighter wants to do this tick. Produced by the keyboard/mouse
/// for the player and by the AI for bots; the sim treats both the same.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Input {
    /// -1 left, 0 none, 1 right.
    pub move_x: f32,
    /// Jump was pressed this tick (edge).
    pub jump: bool,
    /// Jump is held (releasing early cuts the jump short).
    pub jump_held: bool,
    pub crouch: bool,
    /// Aim target in world tiles.
    pub aim: Vec2,
    pub fire: bool,
    pub reload: bool,
    /// Weapon-bar slot to switch to (0-based).
    pub select: Option<usize>,
    /// -1 / +1 to cycle owned weapons.
    pub cycle: i32,
}
