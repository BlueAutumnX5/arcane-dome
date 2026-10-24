//! The map being edited. It is kept as the map file's own JSON, so the editor
//! never drops a field it does not know about (theme, decor, bot hints), and
//! every edit can be undone.

use macroquad::math::{Vec2, vec2};
use serde_json::{Map, Value, json};

/// The kinds of placed things, each a list in the map file.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Layer {
    Spawns,
    Pickups,
    Objects,
}

impl Layer {
    pub const ALL: [Layer; 3] = [Layer::Spawns, Layer::Pickups, Layer::Objects];

    fn key(self) -> &'static str {
        match self {
            Layer::Spawns => "spawns",
            Layer::Pickups => "pickups",
            Layer::Objects => "objects",
        }
    }
}

const UNDO_LIMIT: usize = 200;

pub struct Doc {
    file: Map<String, Value>,
    undo: Vec<Map<String, Value>>,
    redo: Vec<Map<String, Value>>,
    /// Changed since the last save.
    pub dirty: bool,
}

impl Doc {
    /// Open a map file. Only shape maps can be edited.
    pub fn from_json(text: &str) -> Result<Self, String> {
        let file: Map<String, Value> =
            serde_json::from_str(text).map_err(|e| format!("not a map file: {e}"))?;
        if file.contains_key("grid") {
            return Err("old grid maps can't be edited, only shape maps".into());
        }
        if !file.contains_key("size") {
            return Err("map file has no size".into());
        }
        Ok(Self {
            file,
            undo: Vec::new(),
            redo: Vec::new(),
            dirty: false,
        })
    }

    /// An empty map with a floor and the fewest spawns a map may have, ready to
    /// build on.
    pub fn blank(id: &str, name: &str, width: f32, height: f32) -> Self {
        let floor = height - 4.0;
        // Enough for the loader's rules: 4 free-for-all spawns, 2 per team.
        let mut spawns = Vec::new();
        for k in 1..=4 {
            spawns.push(json!({"x": num(width * k as f32 / 5.0), "y": floor}));
        }
        for x in [3.0, 5.0] {
            spawns.push(json!({"x": x, "y": floor, "team": 0}));
            spawns.push(json!({"x": width - x, "y": floor, "team": 1}));
        }
        let value = json!({
            "id": id,
            "name": name,
            "description": "",
            "tile_size": 32,
            "size": [width, height],
            "theme": {"sky_top": "#1d2b4a", "sky_bottom": "#7fa6c9", "far": "#3d5070",
                      "skyline": "mountains"},
            "modes": ["deathmatch", "elimination"],
            "recommended_players": [2, 8],
            "shapes": [
                {"kind": "solid", "material": "rock",
                 "points": [[0, floor], [width, floor], [width, height], [0, height]]}
            ],
            "team_counts": [2],
            "spawns": spawns,
            "pickups": [],
            "objects": [],
        });
        let Value::Object(file) = value else {
            unreachable!()
        };
        Self {
            file,
            undo: Vec::new(),
            redo: Vec::new(),
            dirty: true,
        }
    }

    pub fn id(&self) -> &str {
        self.file.get("id").and_then(Value::as_str).unwrap_or("")
    }

    pub fn size(&self) -> Vec2 {
        let s = self.file.get("size").and_then(Value::as_array);
        let at = |i| {
            s.and_then(|s| s.get(i))
                .and_then(Value::as_f64)
                .unwrap_or(1.0) as f32
        };
        vec2(at(0), at(1))
    }

    /// Call once before each edit (a drag counts as one edit), so undo goes
    /// back to how the map was before it.
    pub fn checkpoint(&mut self) {
        self.undo.push(self.file.clone());
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.dirty = true;
    }

    pub fn undo(&mut self) -> bool {
        match self.undo.pop() {
            Some(prev) => {
                self.redo.push(std::mem::replace(&mut self.file, prev));
                self.dirty = true;
                true
            }
            None => false,
        }
    }

    pub fn redo(&mut self) -> bool {
        match self.redo.pop() {
            Some(next) => {
                self.undo.push(std::mem::replace(&mut self.file, next));
                self.dirty = true;
                true
            }
            None => false,
        }
    }

    fn list(&self, key: &str) -> &[Value] {
        self.file
            .get(key)
            .and_then(Value::as_array)
            .map_or(&[], Vec::as_slice)
    }

    fn list_mut(&mut self, key: &str) -> &mut Vec<Value> {
        let v = self
            .file
            .entry(key)
            .or_insert_with(|| Value::Array(Vec::new()));
        if !v.is_array() {
            *v = Value::Array(Vec::new());
        }
        v.as_array_mut().expect("just made an array")
    }

    // --- Shapes ---------------------------------------------------------

    pub fn shape_count(&self) -> usize {
        self.list("shapes").len()
    }

    pub fn shape(&self, i: usize) -> &Value {
        &self.list("shapes")[i]
    }

    pub fn shape_points(&self, i: usize) -> Vec<Vec2> {
        self.shape(i)
            .get("points")
            .and_then(Value::as_array)
            .map(|pts| pts.iter().filter_map(point).collect())
            .unwrap_or_default()
    }

    pub fn set_shape_points(&mut self, i: usize, pts: &[Vec2]) {
        if let Some(s) = self.list_mut("shapes")[i].as_object_mut() {
            s.insert("points".into(), points_json(pts));
        }
    }

    /// Change one field of a shape (kind, material); `None` removes it.
    pub fn set_shape_field(&mut self, i: usize, field: &str, value: Option<Value>) {
        if let Some(s) = self.list_mut("shapes")[i].as_object_mut() {
            match value {
                Some(v) => {
                    s.insert(field.into(), v);
                }
                None => {
                    s.remove(field);
                }
            }
        }
    }

    pub fn add_shape(&mut self, kind: &str, material: &str, pts: &[Vec2]) -> usize {
        let shapes = self.list_mut("shapes");
        shapes.push(json!({"kind": kind, "material": material, "points": points_json(pts)}));
        shapes.len() - 1
    }

    pub fn remove_shape(&mut self, i: usize) {
        self.list_mut("shapes").remove(i);
    }

    // --- Spawns, pickups, objects ------------------------------------------

    pub fn count(&self, layer: Layer) -> usize {
        self.list(layer.key()).len()
    }

    pub fn thing(&self, layer: Layer, i: usize) -> &Value {
        &self.list(layer.key())[i]
    }

    pub fn thing_pos(&self, layer: Layer, i: usize) -> Vec2 {
        let t = self.thing(layer, i);
        let at = |k| t.get(k).and_then(Value::as_f64).unwrap_or(0.0) as f32;
        vec2(at("x"), at("y"))
    }

    pub fn set_thing_pos(&mut self, layer: Layer, i: usize, p: Vec2) {
        if let Some(t) = self.list_mut(layer.key())[i].as_object_mut() {
            t.insert("x".into(), num(p.x));
            t.insert("y".into(), num(p.y));
        }
    }

    /// Change one field of a placed thing; `None` removes it.
    pub fn set_thing_field(&mut self, layer: Layer, i: usize, field: &str, value: Option<Value>) {
        if let Some(t) = self.list_mut(layer.key())[i].as_object_mut() {
            match value {
                Some(v) => {
                    t.insert(field.into(), v);
                }
                None => {
                    t.remove(field);
                }
            }
        }
    }

    /// Add a thing; `fields` holds everything but its position.
    pub fn add_thing(&mut self, layer: Layer, p: Vec2, fields: Value) -> usize {
        let mut t = Map::new();
        if let Value::Object(f) = fields {
            for (k, v) in f {
                if k == "x" || k == "y" {
                    continue;
                }
                t.insert(k, v);
            }
        }
        // x and y first, after the type, like the hand-built maps.
        let mut out = Map::new();
        if let Some(kind) = t.remove("type") {
            out.insert("type".into(), kind);
        }
        out.insert("x".into(), num(p.x));
        out.insert("y".into(), num(p.y));
        out.extend(t);
        let list = self.list_mut(layer.key());
        list.push(Value::Object(out));
        list.len() - 1
    }

    pub fn remove_thing(&mut self, layer: Layer, i: usize) {
        self.list_mut(layer.key()).remove(i);
    }

    /// The map file text: one line per top-level field and per list entry,
    /// like the maps the builder scripts write.
    pub fn to_json(&self) -> String {
        let mut out = String::from("{\n");
        let n = self.file.len();
        for (i, (k, v)) in self.file.iter().enumerate() {
            out.push_str(&format!("  {}: ", Value::String(k.clone())));
            match v {
                Value::Array(items) if items.iter().any(Value::is_object) => {
                    out.push_str("[\n");
                    for (j, item) in items.iter().enumerate() {
                        out.push_str("    ");
                        out.push_str(&compact(item));
                        out.push_str(if j + 1 < items.len() { ",\n" } else { "\n" });
                    }
                    out.push_str("  ]");
                }
                _ => out.push_str(&compact(v)),
            }
            out.push_str(if i + 1 < n { ",\n" } else { "\n" });
        }
        out.push_str("}\n");
        out
    }
}

/// JSON on one line with a space after `:` and `,`, as the builder writes it.
fn compact(v: &Value) -> String {
    match v {
        Value::Array(a) => format!("[{}]", a.iter().map(compact).collect::<Vec<_>>().join(", ")),
        Value::Object(o) => format!(
            "{{{}}}",
            o.iter()
                .map(|(k, v)| format!("{}: {}", Value::String(k.clone()), compact(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        _ => v.to_string(),
    }
}

fn point(v: &Value) -> Option<Vec2> {
    let a = v.as_array()?;
    Some(vec2(
        a.first()?.as_f64()? as f32,
        a.get(1)?.as_f64()? as f32,
    ))
}

/// A coordinate rounded to 1/100 tile, written as an integer when whole.
fn num(x: f32) -> Value {
    let r = (x as f64 * 100.0).round() / 100.0;
    if r.fract() == 0.0 && r.abs() < 1e9 {
        json!(r as i64)
    } else {
        json!(r)
    }
}

fn points_json(pts: &[Vec2]) -> Value {
    Value::Array(pts.iter().map(|p| json!([num(p.x), num(p.y)])).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped(name: &str) -> String {
        std::fs::read_to_string(format!(
            "{}/assets/data/maps/{name}.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    }

    #[test]
    fn saving_unchanged_maps_keeps_every_field() {
        for name in ["foundry_ascent", "frostline_relay", "redstone_arches"] {
            let text = shipped(name);
            let doc = Doc::from_json(&text).unwrap();
            let before: Value = serde_json::from_str(&text).unwrap();
            let after: Value = serde_json::from_str(&doc.to_json()).unwrap();
            assert_eq!(before, after, "{name} changed on a plain save");
        }
    }

    #[test]
    fn undo_and_redo_walk_the_history() {
        let mut doc = Doc::blank("test", "Test", 40.0, 20.0);
        let start = doc.to_json();
        doc.checkpoint();
        doc.set_thing_pos(Layer::Spawns, 0, vec2(10.0, 16.0));
        let moved = doc.to_json();
        doc.checkpoint();
        doc.add_shape(
            "hatch",
            "wood",
            &[vec2(1.0, 1.0), vec2(3.0, 1.0), vec2(3.0, 1.25)],
        );
        assert_eq!(doc.shape_count(), 2);
        assert!(doc.undo());
        assert_eq!(doc.to_json(), moved);
        assert!(doc.undo());
        assert_eq!(doc.to_json(), start);
        assert!(!doc.undo());
        assert!(doc.redo());
        assert_eq!(doc.to_json(), moved);
        // A new edit after undo drops the redo history.
        doc.checkpoint();
        doc.remove_thing(Layer::Spawns, 1);
        assert!(!doc.redo());
    }

    #[test]
    fn added_things_put_type_and_position_first() {
        let mut doc = Doc::blank("test", "Test", 40.0, 20.0);
        let i = doc.add_thing(
            Layer::Objects,
            vec2(12.5, 16.0),
            json!({"height_tiles": 8, "type": "thruster", "x": 99}),
        );
        assert_eq!(
            compact(doc.thing(Layer::Objects, i)),
            r#"{"type": "thruster", "x": 12.5, "y": 16, "height_tiles": 8}"#
        );
        assert_eq!(doc.thing_pos(Layer::Objects, i), vec2(12.5, 16.0));
    }

    #[test]
    fn a_blank_map_loads_in_the_game() {
        let doc = Doc::blank("blank_test", "Blank", 40.0, 20.0);
        let map = crate::data::parse_map(
            "blank_test.json",
            "blank_test",
            &doc.to_json(),
            &[],
            &["deathmatch", "elimination"],
        )
        .unwrap_or_else(|e| panic!("{e:?}"));
        assert_eq!(map.spawns.len(), 8);
    }

    #[test]
    fn grid_maps_are_refused() {
        assert!(Doc::from_json(r##"{"id": "x", "size": [9, 9], "grid": ["#"]}"##).is_err());
        assert!(Doc::from_json("not json").is_err());
    }
}
