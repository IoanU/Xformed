//! A source-derived visual genome chooses composition, not merely a different noise offset.
use crate::mood::{mixed, Atmosphere};
use image::{Rgb, RgbImage};
use std::f32::consts::{PI, TAU};
fn rnd(seed: u64, n: u64) -> f32 {
    (mixed(seed.wrapping_add(n.wrapping_mul(0x9e3779b97f4a7c15))) >> 40) as f32 / 16777215.0
}
fn unit(x: i32, y: i32, seed: u64) -> f32 {
    rnd(seed ^ ((x as u32 as u64) << 32) ^ y as u32 as u64, 7)
}
fn noise(x: f32, y: f32, seed: u64) -> f32 {
    let (ix, iy) = (x.floor() as i32, y.floor() as i32);
    let (fx, fy) = (x - ix as f32, y - iy as f32);
    let (u, v) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let a = unit(ix, iy, seed);
    let b = unit(ix + 1, iy, seed);
    let c = unit(ix, iy + 1, seed);
    let d = unit(ix + 1, iy + 1, seed);
    (a + (b - a) * u) * (1.0 - v) + (c + (d - c) * u) * v
}
fn fbm(mut x: f32, mut y: f32, seed: u64, lacunarity: f32, persistence: f32) -> f32 {
    let (mut sum, mut weight, mut norm) = (0.0, 1.0, 0.0);
    for i in 0..5 {
        sum += noise(x, y, seed.wrapping_add(i * 193)) * weight;
        norm += weight;
        weight *= persistence;
        x = x * lacunarity + 5.1;
        y = y * lacunarity - 3.2;
    }
    sum / norm
}
fn hsv(h: f32, s: f32, v: f32) -> [f32; 3] {
    let h = h.rem_euclid(360.0) / 60.0;
    let c = v * s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let m = v - c;
    let q = match h as u32 {
        0 => [c, x, 0.0],
        1 => [x, c, 0.0],
        2 => [0.0, c, x],
        3 => [0.0, x, c],
        4 => [x, 0.0, c],
        _ => [c, 0.0, x],
    };
    q.map(|a| (a + m) * 255.0)
}
fn palette(colors: &[[f32; 3]; 5], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 0.9999) * 4.0;
    let i = t as usize;
    let f = t - i as f32;
    std::array::from_fn(|j| colors[i][j] * (1.0 - f) + colors[i + 1][j] * f)
}
pub fn identity(a: &Atmosphere, style: &str) -> String {
    let family = (mixed(a.seed ^ 0xb5297a4d) % 4) as usize;
    let name = match style {
        "bloom" => ["Corolla", "Archipelago", "Phyllotaxis", "Coral"][family],
        "contour" => ["Relief", "Strata", "Interference", "Cells"][family],
        _ => ["Plume", "Orbit", "Tide", "Rift"][family],
    };
    format!(
        "{} / {name}",
        match style {
            "bloom" => "Bloom",
            "contour" => "Contour",
            _ => "Nebula",
        }
    )
}
/// Successive takes explore all four composition systems before repeating one.
pub fn variation(a: &Atmosphere, take: u32) -> Atmosphere {
    let mut result = a.variant(take);
    let target = mixed(a.seed ^ 0xb5297a4d).wrapping_add(take as u64) % 4;
    while mixed(result.seed ^ 0xb5297a4d) % 4 != target {
        result.seed = mixed(result.seed);
    }
    result
}
fn source_contour(values: &[f32], position: f32) -> f32 {
    if values.is_empty() {
        return 0.5;
    }
    let x = position.clamp(0.0, 1.0) * values.len().saturating_sub(1) as f32;
    let i = x.floor() as usize;
    let t = x.fract();
    let t = t * t * (3.0 - 2.0 * t);
    values[i] * (1.0 - t) + values[(i + 1).min(values.len() - 1)] * t
}
pub fn render(a: &Atmosphere, style: &str) -> RgbImage {
    render_at(a, style, 1280, 832)
}
fn render_at(a: &Atmosphere, style: &str, w: u32, h: u32) -> RgbImage {
    let seed = a.seed;
    let family = (mixed(seed ^ 0xb5297a4d) % 4) as usize;
    let mut img = RgbImage::new(w, h);
    let angle = rnd(seed, 2) * TAU;
    let (sn, cs) = angle.sin_cos();
    let scale = 0.65 + rnd(seed, 3) * 2.5 + a.energy * 0.6;
    let cx = (rnd(seed, 4) - 0.5) * 1.2;
    let cy = (rnd(seed, 5) - 0.5) * 0.7;
    let warp = 0.25 + rnd(seed, 6) * 3.8 + a.energy * 1.7;
    let lac = 1.65 + rnd(seed, 8) * 1.1;
    let persistence = (0.3 + rnd(seed, 9) * 0.2 + a.complexity * 0.16).clamp(0.3, 0.68);
    let arms = 3 + (rnd(seed, 10) * 9.0) as usize;
    let twist = (rnd(seed, 11) - 0.5) * 15.0;
    let zoom = 0.6 + rnd(seed, 12) * 1.8;
    let mut points = Vec::new();
    for i in 0..(5 + (rnd(seed, 13) * 9.0) as usize) {
        points.push((
            (rnd(seed, 100 + i as u64 * 2) - 0.5) * 3.5,
            (rnd(seed, 101 + i as u64 * 2) - 0.5) * 2.5,
        ));
    }
    let hue = (a.hue * 0.35 + rnd(seed, 14) * 300.0 + a.valence * 35.0).rem_euclid(360.0);
    let harmony = match (mixed(seed ^ 444) % 4) as usize {
        0 => [0.0, 25.0, 55.0],
        1 => [0.0, 160.0, 205.0],
        2 => [0.0, 115.0, 235.0],
        _ => [0.0, 0.0, 25.0],
    };
    let sat = 0.45 + rnd(seed, 15) * 0.48;
    let mut colors = [
        hsv(hue, 0.55, 0.035),
        hsv(hue + harmony[0], sat, 0.27),
        hsv(hue + harmony[1], sat * 0.85, 0.58),
        hsv(hue + harmony[2], sat * 0.7, 0.9),
        hsv(hue + harmony[2] + 20.0, 0.12, 1.0),
    ];
    let paper = style == "contour" && rnd(seed, 16) > 0.55;
    if paper {
        colors.reverse();
    }
    let js = [
        (-0.745, 0.113),
        (0.285, 0.01),
        (-0.4, 0.6),
        (-0.8, 0.156),
        (-0.123, 0.745),
        (-0.70176, -0.3842),
        (0.355, 0.355),
    ];
    let jc = js[(mixed(seed ^ 91) % js.len() as u64) as usize];
    let frequency = 4.0 + rnd(seed, 17) * 19.0;
    let contrast = 0.85 + rnd(seed, 18) * 0.9;
    let stretch = 0.65 + rnd(seed, 24) * 0.9;
    let fullness = rnd(seed, 25);
    for (px, py, pixel) in img.enumerate_pixels_mut() {
        let x = (px as f32 / w as f32 - 0.5) * 3.4 - cx;
        let y = (py as f32 / h as f32 - 0.5) * 2.2 - cy;
        let rx = x * cs - y * sn;
        let ry = x * sn + y * cs;
        let r = (rx * rx + ry * ry).sqrt();
        let theta = ry.atan2(rx);
        let nx = rx * scale;
        let ny = ry * scale;
        let qx = fbm(nx + 1.1, ny - 3.7, seed, lac, persistence);
        let qy = fbm(nx - 8.3, ny + 2.4, seed ^ 71, lac, persistence);
        let wx = nx + (qx - 0.5) * warp;
        let wy = ny + (qy - 0.5) * warp;
        let f = fbm(wx, wy, seed ^ 937, lac, persistence);
        let contour = source_contour(&a.contour, px as f32 / w as f32);
        let bx = rx * stretch + (qx - 0.5) * 0.18;
        let by = ry / stretch + (qy - 0.5) * 0.18;
        let br = (bx * bx + by * by).sqrt();
        let bt = by.atan2(bx);
        let (v, edge) = match (style, family) {
            ("nebula", 0) => {
                let envelope =
                    (-((rx * 0.55 + 0.3).powi(2) + (ry * 1.1 + (qx - 0.5) * 0.6).powi(2)) * 1.6)
                        .exp();
                let cloud = ((f - 0.25) * 1.9).max(0.0) * envelope;
                (
                    cloud,
                    (1.0 - (f * 24.0 + ry * 5.0).sin().abs()).powf(16.0) * envelope,
                )
            }
            ("nebula", 1) => {
                let spiral = (theta * arms as f32 + r * twist + f * 3.0).sin();
                let ring =
                    (-((r - (0.55 + rnd(seed, 21) * 0.4) - (qx - 0.5) * 0.2) * 5.0).powi(2)).exp();
                let core = (-r * r * 10.0).exp();
                (
                    ring * (0.4 + spiral.abs() * 0.5) + core * 0.45,
                    ring * (1.0 - spiral.abs()).powf(6.0),
                )
            }
            ("nebula", 2) => {
                let wave = ry * frequency + (rx * 1.4 + f * 2.0).sin() * warp + contour * 2.0;
                let sheets = (wave.sin() * 0.5 + 0.5).powf(2.2);
                (
                    sheets * (0.45 + f * 0.5),
                    (1.0 - wave.sin().abs()).powf(18.0),
                )
            }
            ("nebula", _) => {
                let fault = (rx * 0.8 + (ry * 2.0).sin() * 0.3 + (f - 0.5) * warp * 0.5).abs();
                let streak = (-fault * (8.0 + frequency)).exp();
                (streak * 0.8 + f * 0.16, streak * (wy * 15.0).sin().abs())
            }
            ("bloom", 0) => {
                let petal = 0.5 + (0.1 + fullness * 0.25) * (bt * arms as f32 + twist * 0.15).cos();
                let ring =
                    ((br - petal * (0.75 + qx * 0.5)).abs() * (4.0 + fullness * 7.0)).min(1.0);
                let shells = (br * frequency + (bt * arms as f32).sin() * 1.4 + f).sin();
                let inside = (1.0 - br / petal.max(0.1)).clamp(0.0, 1.0);
                (
                    (1.0 - ring) * 0.65 + inside * (0.15 + fullness * 0.6),
                    (1.0 - shells.abs()).powf(20.0) * (1.0 - ring),
                )
            }
            ("bloom", 1) => {
                let mut zx = rx * zoom + (qx - 0.5) * 0.12;
                let mut zy = ry * zoom + (qy - 0.5) * 0.12;
                let mut trap = 2.0f32;
                let mut count = 0;
                for _ in 0..(30 + (rnd(seed, 22) * 25.0) as usize) {
                    let xx = zx * zx - zy * zy + jc.0;
                    zy = 2.0 * zx * zy + jc.1;
                    zx = xx;
                    trap = trap.min(zx.abs() + (zy * 2.0).sin().abs() * 0.2);
                    count += 1;
                    if zx * zx + zy * zy > 25.0 {
                        break;
                    }
                }
                (
                    (count as f32 / 48.0 * 0.5 + (-trap * 8.0).exp() * 0.35 + f * 0.15).min(1.0),
                    (-trap * 20.0).exp() * 0.7,
                )
            }
            ("bloom", 2) => {
                let spiral =
                    (bt * arms as f32 + (br + 0.04).ln() * (7.0 + twist.abs()) + f * 2.0).sin();
                let envelope = (-br * br * (0.4 + fullness)).exp();
                let bead_mask = (br * (25.0 + frequency * 2.0) + bt.sin() * 2.0).sin().abs();
                let beads = (1.0 - spiral.abs()).powf(1.5 + fullness * 4.0)
                    * if fullness > 0.38 {
                        bead_mask.powf(3.0)
                    } else {
                        1.0
                    };
                (
                    (beads * 0.9 + f * (0.12 + fullness * 0.3)) * envelope,
                    beads.powf(4.0) * envelope,
                )
            }
            ("bloom", _) => {
                let mut zx = rx * 0.8;
                let mut zy = ry * 0.8;
                let mut trap = 2.0f32;
                for k in 0..6 {
                    zx = zx.abs() * 1.45 - (0.6 + rnd(seed, 30 + k) * 0.3);
                    zy = zy.abs() * 1.6 - (0.4 + rnd(seed, 40 + k) * 0.2);
                    let xx = zx * 0.86 - zy * 0.5;
                    zy = zx * 0.5 + zy * 0.86;
                    zx = xx;
                    trap = trap.min((zx * zx + zy * zy).sqrt());
                }
                let branch = (-trap * 5.0).exp();
                (
                    (branch * 0.8 + f * 0.15) * (-r * r * 0.2).exp(),
                    (-trap * 16.0).exp(),
                )
            }
            ("contour", 0) => {
                let height = f * 1.3 + rx * 0.12 + contour * 0.12;
                let line = (1.0 - (height * frequency * PI).sin().abs()).powf(16.0);
                (height * 0.65, line)
            }
            ("contour", 1) => {
                let height = ry * 0.5 + (rx * 1.2 + qx * 3.0).sin() * 0.23 + f * 0.55;
                let bands = (height * frequency).rem_euclid(1.0);
                (
                    (height * 0.3 + 0.5 + ((height * frequency).floor() % 3.0) * 0.08)
                        .clamp(0.0, 1.0),
                    (1.0 - bands).powf(18.0),
                )
            }
            ("contour", 2) => {
                let mut waves = 0.0;
                for (i, (xx, yy)) in points.iter().take(4).enumerate() {
                    let d = ((rx - xx).powi(2) + (ry - yy).powi(2)).sqrt();
                    waves += (d * frequency + i as f32 * 1.7).sin();
                }
                let height = waves / 4.0;
                (
                    (height * 0.4 + 0.5) * 0.8,
                    (1.0 - (height * frequency).sin().abs()).powf(15.0),
                )
            }
            _ => {
                let mut d1 = 100.0f32;
                let mut d2 = 100.0f32;
                for (xx, yy) in &points {
                    let d = (rx - xx + (qx - 0.5) * 0.12).powi(2)
                        + (ry - yy + (qy - 0.5) * 0.12).powi(2);
                    if d < d1 {
                        d2 = d1;
                        d1 = d;
                    } else if d < d2 {
                        d2 = d;
                    }
                }
                let boundary = (-(d2 - d1) * 22.0).exp();
                (d1.sqrt().min(1.0) * 0.6 + f * 0.15, boundary)
            }
        };
        let t = ((v - 0.42) * contrast + 0.43 + edge * 0.18).clamp(0.0, 1.0);
        let mut rgb = palette(&colors, t);
        let vignette = if paper {
            1.0
        } else {
            (1.0 - (rx * rx * 0.05 + ry * ry * 0.08)).clamp(0.55, 1.0)
        };
        let grain = (unit(px as i32, py as i32, seed ^ 555) - 0.5) * (0.5 + rnd(seed, 23) * 2.0);
        for j in 0..3 {
            rgb[j] = (rgb[j] + edge * colors[4][j] * 0.11) * vignette + grain;
        }
        *pixel = Rgb(rgb.map(|v| v.clamp(0.0, 255.0) as u8));
    }
    img
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn noise_is_smooth_and_seeded() {
        let a = noise(1.2, 3.4, 42);
        assert!((a - noise(1.201, 3.4, 42)).abs() < 0.005);
        assert_ne!(a, noise(1.2, 3.4, 43));
    }
    #[test]
    fn successive_takes_explore_different_compositions() {
        let a = crate::mood::text_atmosphere("A quiet morning");
        let names: std::collections::HashSet<_> = (0..4)
            .map(|take| identity(&variation(&a, take), "nebula"))
            .collect();
        assert_eq!(names.len(), 4);
        assert_eq!(variation(&a, 0).seed, a.seed);
    }
    #[test]
    fn geometry_changes_within_each_style() {
        let base = crate::mood::text_atmosphere("A quiet morning");
        for style in ["nebula", "bloom", "contour"] {
            let mut families = std::collections::HashSet::new();
            let mut profiles = Vec::new();
            for seed in 0..12 {
                let mut a = base.clone();
                a.seed = mixed(seed);
                families.insert(identity(&a, style));
                let img = render_at(&a, style, 96, 64);
                let v: Vec<f32> = img
                    .pixels()
                    .map(|p| p[0] as f32 * 0.2126 + p[1] as f32 * 0.7152 + p[2] as f32 * 0.0722)
                    .collect();
                profiles.push(v);
            }
            assert!(families.len() >= 3);
            let pairs =
                (0..profiles.len()).flat_map(|i| (i + 1..profiles.len()).map(move |j| (i, j)));
            let mut mean = 0.0;
            let mut n = 0;
            for (i, j) in pairs {
                let x = &profiles[i];
                let y = &profiles[j];
                let mx = x.iter().sum::<f32>() / x.len() as f32;
                let my = y.iter().sum::<f32>() / y.len() as f32;
                let cov = x
                    .iter()
                    .zip(y)
                    .map(|(a, b)| (a - mx) * (b - my))
                    .sum::<f32>();
                let sx = x.iter().map(|a| (a - mx).powi(2)).sum::<f32>();
                let sy = y.iter().map(|a| (a - my).powi(2)).sum::<f32>();
                mean += (cov / (sx * sy).sqrt().max(1e-6)).abs();
                n += 1;
            }
            assert!(
                (mean / n as f32) < 0.60,
                "{style} grayscale structure correlation {}",
                mean / n as f32
            );
        }
    }
}
