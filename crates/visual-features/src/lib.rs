
use anyhow::Result;
use image::GenericImageView;
use palette::{FromColor, Hsv, Srgb};

#[derive(Debug, Clone, Copy)]
pub struct HsvStats { pub h: f32, pub s: f32, pub v: f32 }

/// Approximate dominant HSV by uniform subsampling and averaging
pub fn dominant_hsv(img_bytes: &[u8]) -> Result<HsvStats> {
    let img = image::load_from_memory(img_bytes)?;
    let (w,_h) = img.dimensions();
    let mut acc_h = 0f32; let mut acc_s = 0f32; let mut acc_v = 0f32; let mut n=0f32;

    // Sample every ~50th pixel in x for speed
    let step = (w / 200).max(1); // adapt sampling
    for (x, _y, p) in img.pixels() {
        if (x % step) != 0 { continue; }
        let srgb = Srgb::new(p.0[0] as f32/255.0, p.0[1] as f32/255.0, p.0[2] as f32/255.0);
        let hsv: Hsv = Hsv::from_color(srgb);
        acc_h += hsv.hue.into_degrees() as f32;
        acc_s += hsv.saturation;
        acc_v += hsv.value;
        n += 1.0;
    }
    if n == 0.0 { n = 1.0; }
    Ok(HsvStats{ h: acc_h/n, s: acc_s/n, v: acc_v/n })
}
