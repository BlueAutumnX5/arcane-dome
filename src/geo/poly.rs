//! Small polygon helpers.

use macroquad::math::Vec2;

/// Even-odd point-in-polygon test.
pub fn point_in_polygon(p: Vec2, pts: &[Vec2]) -> bool {
    let n = pts.len();
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (pts[i], pts[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn cross(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

/// Distance along the ray `origin + dir * t` to segment `a..b`, if it hits.
pub fn ray_segment(origin: Vec2, dir: Vec2, a: Vec2, b: Vec2) -> Option<f32> {
    let e = b - a;
    let denom = cross(dir, e);
    if denom.abs() < 1e-9 {
        return None;
    }
    let ao = a - origin;
    let t = cross(ao, e) / denom;
    let u = cross(ao, dir) / denom;
    (t >= 0.0 && (0.0..=1.0).contains(&u)).then_some(t)
}

/// Does segment `a..b` touch the box `min..max`? (Liang-Barsky clip.)
pub fn segment_hits_box(a: Vec2, b: Vec2, min: Vec2, max: Vec2) -> bool {
    let d = b - a;
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for (p, q) in [
        (-d.x, a.x - min.x),
        (d.x, max.x - a.x),
        (-d.y, a.y - min.y),
        (d.y, max.y - a.y),
    ] {
        if p.abs() < 1e-12 {
            if q < 0.0 {
                return false;
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                t0 = t0.max(r);
            } else {
                t1 = t1.min(r);
            }
            if t0 > t1 {
                return false;
            }
        }
    }
    true
}

/// Ear-clipping triangulation of a simple polygon (for drawing).
pub fn triangulate(pts: &[Vec2]) -> Vec<[Vec2; 3]> {
    let n = pts.len();
    if n < 3 {
        return Vec::new();
    }
    let area: f32 = (0..n).map(|i| cross(pts[i], pts[(i + 1) % n])).sum::<f32>() * 0.5;
    let mut idx: Vec<usize> = (0..n).collect();
    if area < 0.0 {
        idx.reverse();
    }
    let mut tris = Vec::new();
    let mut guard = 0;
    while idx.len() > 3 && guard < n * n {
        guard += 1;
        let m = idx.len();
        let mut clipped = false;
        for i in 0..m {
            let (ia, ib, ic) = (idx[(i + m - 1) % m], idx[i], idx[(i + 1) % m]);
            let (a, b, c) = (pts[ia], pts[ib], pts[ic]);
            if cross(b - a, c - b) <= 1e-9 {
                continue; // reflex or flat corner
            }
            let contains = idx
                .iter()
                .any(|&j| j != ia && j != ib && j != ic && in_triangle(pts[j], a, b, c));
            if !contains {
                tris.push([a, b, c]);
                idx.remove(i);
                clipped = true;
                break;
            }
        }
        if !clipped {
            break;
        }
    }
    if idx.len() == 3 {
        tris.push([pts[idx[0]], pts[idx[1]], pts[idx[2]]]);
    }
    tris
}

fn in_triangle(p: Vec2, a: Vec2, b: Vec2, c: Vec2) -> bool {
    let d1 = cross(b - a, p - a);
    let d2 = cross(c - b, p - b);
    let d3 = cross(a - c, p - c);
    d1 >= 0.0 && d2 >= 0.0 && d3 >= 0.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use macroquad::math::vec2;

    #[test]
    fn point_in_concave_polygon() {
        // An L shape.
        let l = [
            vec2(0.0, 0.0),
            vec2(4.0, 0.0),
            vec2(4.0, 1.0),
            vec2(1.0, 1.0),
            vec2(1.0, 4.0),
            vec2(0.0, 4.0),
        ];
        assert!(point_in_polygon(vec2(0.5, 3.0), &l));
        assert!(!point_in_polygon(vec2(3.0, 3.0), &l));
    }

    #[test]
    fn triangulation_covers_the_area() {
        let l = [
            vec2(0.0, 0.0),
            vec2(4.0, 0.0),
            vec2(4.0, 1.0),
            vec2(1.0, 1.0),
            vec2(1.0, 4.0),
            vec2(0.0, 4.0),
        ];
        let tris = triangulate(&l);
        let area: f32 = tris
            .iter()
            .map(|t| cross(t[1] - t[0], t[2] - t[0]).abs() * 0.5)
            .sum();
        assert!((area - 7.0).abs() < 1e-4, "{area}");
    }

    #[test]
    fn rays_and_boxes() {
        let t = ray_segment(
            vec2(0.0, 0.0),
            vec2(1.0, 0.0),
            vec2(3.0, -1.0),
            vec2(3.0, 1.0),
        );
        assert_eq!(t, Some(3.0));
        assert!(segment_hits_box(
            vec2(-1.0, 0.5),
            vec2(5.0, 0.5),
            vec2(0.0, 0.0),
            vec2(1.0, 1.0)
        ));
        assert!(!segment_hits_box(
            vec2(-1.0, 2.0),
            vec2(5.0, 2.0),
            vec2(0.0, 0.0),
            vec2(1.0, 1.0)
        ));
    }
}
