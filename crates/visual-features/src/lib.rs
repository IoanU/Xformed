// crates/visual-features/src/lib.rs
use anyhow::Result;
use serde::{Serialize, Deserialize};
use palette::{Srgb, IntoColor, Hsv};
use image::{DynamicImage, GenericImageView};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageFeatures {
    pub width: u32,
    pub height: u32,
    pub aspect_ratio: f32,

    pub hsv_mean_h: f32, // [0,360)
    pub hsv_mean_s: f32, // [0,1]
    pub hsv_mean_v: f32, // [0,1]
    pub hue_variance: f32,

    pub colorfulness_hs: f32, // Hasler–Süsstrunk
    pub contrast_luma_std: f32,
    pub entropy_luma_bits: f32, // 0..8

    pub edge_density: f32, // [0,1]
}

pub fn analyze_image_bytes(img_bytes: &[u8]) -> Result<ImageFeatures> {
    let img = image::load_from_memory(img_bytes)?;
    analyze_image(&img)
}

pub fn analyze_image(img: &DynamicImage) -> Result<ImageFeatures> {
    let (w,h) = img.dimensions();
    let aspect = w as f32 / h.max(1) as f32;

    // Downscale pentru viteză
    let small = img.thumbnail(256, 256).to_rgb8();
    let mut sum_h = 0.0f32; let mut sum_s = 0.0f32; let mut sum_v = 0.0f32;
    let mut hs: Vec<f32> = Vec::with_capacity((small.width()*small.height()) as usize);

    // Hasler–Süsstrunk colorfulness
    let mut rg = Vec::new(); let mut yb = Vec::new();

    // Luma pentru contrast/entropie/edges
    let gray = img.thumbnail(256, 256).to_luma8();
    let mut luma_vals = Vec::with_capacity((gray.width()*gray.height()) as usize);

    for px in small.pixels() {
        let (r,g,b) = (px[0] as f32 / 255.0, px[1] as f32 / 255.0, px[2] as f32 / 255.0);
        let hsv: Hsv = Srgb::new(r,g,b).into_format::<f32>().into_color();
        sum_h += hsv.hue.into_degrees();
        sum_s += hsv.saturation;
        sum_v += hsv.value;
        hs.push(hsv.hue.into_degrees());

        // HS colorfulness auxiliars
        rg.push(r - g);
        yb.push(0.5*(r+g) - b);
    }
    let n = hs.len().max(1) as f32;
    let mean_h = sum_h / n;
    let mean_s = sum_s / n;
    let mean_v = sum_v / n;
    let hue_variance = if hs.is_empty() { 0.0 } else {
        let m = mean_h;
        (hs.iter().map(|&x| {
            let mut d = (x - m).abs();
            // circular wrap
            if d > 180.0 { d = 360.0 - d; }
            d*d
        }).sum::<f32>() / n).sqrt()
    };

    // Hasler–Süsstrunk colorfulness
    fn mean_std(v: &[f32]) -> (f32,f32) {
        if v.is_empty() { return (0.0,0.0); }
        let m = v.iter().sum::<f32>() / v.len() as f32;
        let s = (v.iter().map(|&x|(x-m)*(x-m)).sum::<f32>() / v.len() as f32).sqrt();
        (m,s)
    }
    let (rg_m, rg_s) = mean_std(&rg);
    let (yb_m, yb_s) = mean_std(&yb);
    let colorfulness_hs = ((rg_s.powi(2) + yb_s.powi(2)).sqrt() + 0.3*(rg_m.powi(2)+yb_m.powi(2)).sqrt()) * 100.0;

    // Luma
    for p in gray.pixels() { luma_vals.push(p[0] as f32 / 255.0); }
    let l_mean = luma_vals.iter().sum::<f32>()/luma_vals.len().max(1) as f32;
    let contrast_luma_std = (luma_vals.iter().map(|&x|(x-l_mean)*(x-l_mean)).sum::<f32>()/luma_vals.len().max(1) as f32).sqrt();

    // Entropie 256-bin
    let mut hist = [0usize;256];
    for p in gray.pixels() { hist[p[0] as usize]+=1; }
    let total = (gray.width()*gray.height()) as f64;
    let entropy_luma_bits = if total>0.0 {
        let h: f64 = hist.iter().map(|&c| {
            if c==0 { 0.0 } else {
                let p = c as f64 / total;
                -p * (p.ln()/std::f64::consts::LN_2)
            }
        }).sum();
        h as f32
    } else { 0.0 };

    // Sobel edge density
    let sobel_mag = imageproc::gradients::sobel_gradients(&gray);

    let mut active = 0usize;
    let mut tot = 0usize;

    // Convertim la u8 (clamp 0..255) pentru un prag simplu
    for (_, _, image::Luma([g16])) in sobel_mag.enumerate_pixels() {
        let g = (*g16 as u32).min(255) as u8;
        tot += 1;
        if g > 32 { active += 1; } // prag simplu
    }

    let edge_density = if tot > 0 { active as f32 / tot as f32 } else { 0.0 };

    Ok(ImageFeatures{
        width: w, height: h, aspect_ratio: aspect,
        hsv_mean_h: mean_h.rem_euclid(360.0), hsv_mean_s: mean_s.clamp(0.0,1.0), hsv_mean_v: mean_v.clamp(0.0,1.0),
        hue_variance,
        colorfulness_hs,
        contrast_luma_std,
        entropy_luma_bits,
        edge_density,
    })
}
