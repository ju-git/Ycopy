//! Generates `assets/ycopy.ico`: an arrow splitting into two.
//!
//! Usage: `cargo run --example make_icon`
//! Dependency-free: rasterises with 4x4 supersampling and writes 32-bit BMP icon frames.

use std::fs;

const SIZES: [u32; 4] = [16, 32, 48, 256];
const SS: u32 = 4;

type P = (f32, f32);

fn dist_to_segment(p: P, a: P, b: P) -> f32 {
    let (abx, aby) = (b.0 - a.0, b.1 - a.1);
    let (apx, apy) = (p.0 - a.0, p.1 - a.1);
    let t = ((apx * abx + apy * aby) / (abx * abx + aby * aby)).clamp(0.0, 1.0);
    let (cx, cy) = (a.0 + abx * t, a.1 + aby * t);
    ((p.0 - cx).powi(2) + (p.1 - cy).powi(2)).sqrt()
}

fn in_triangle(p: P, a: P, b: P, c: P) -> bool {
    let s = |p1: P, p2: P, p3: P| (p1.0 - p3.0) * (p2.1 - p3.1) - (p2.0 - p3.0) * (p1.1 - p3.1);
    let (d1, d2, d3) = (s(p, a, b), s(p, b, c), s(p, c, a));
    let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(neg && pos)
}

fn in_rounded_square(p: P, margin: f32, radius: f32) -> bool {
    let (lo, hi) = (margin + radius, 1.0 - margin - radius);
    let cx = p.0.clamp(lo, hi);
    let cy = p.1.clamp(lo, hi);
    p.0 >= margin
        && p.0 <= 1.0 - margin
        && p.1 >= margin
        && p.1 <= 1.0 - margin
        && ((p.0 - cx).powi(2) + (p.1 - cy).powi(2)).sqrt() <= radius
}

fn is_arrow(p: P) -> bool {
    let r = 0.045;
    let split: P = (0.40, 0.5);
    let segs: [(P, P); 5] = [
        ((0.16, 0.5), split),
        (split, (0.54, 0.29)),
        ((0.54, 0.29), (0.70, 0.29)),
        (split, (0.54, 0.71)),
        ((0.54, 0.71), (0.70, 0.71)),
    ];
    if segs.iter().any(|&(a, b)| dist_to_segment(p, a, b) <= r) {
        return true;
    }
    [0.29f32, 0.71].iter().any(|&y| {
        in_triangle(p, (0.68, y - 0.115), (0.68, y + 0.115), (0.86, y))
    })
}

/// Returns BGRA pixels, top-down.
fn render(size: u32) -> Vec<[u8; 4]> {
    let mut out = Vec::with_capacity((size * size) as usize);
    for y in 0..size {
        for x in 0..size {
            let (mut bg, mut fg) = (0u32, 0u32);
            for sy in 0..SS {
                for sx in 0..SS {
                    let p = (
                        (x as f32 + (sx as f32 + 0.5) / SS as f32) / size as f32,
                        (y as f32 + (sy as f32 + 0.5) / SS as f32) / size as f32,
                    );
                    if in_rounded_square(p, 0.03, 0.2) {
                        bg += 1;
                        if is_arrow(p) {
                            fg += 1;
                        }
                    }
                }
            }
            let total = (SS * SS) as f32;
            let alpha = bg as f32 / total;
            if bg == 0 {
                out.push([0, 0, 0, 0]);
                continue;
            }
            // Blend white arrow over blue (#2563EB) within the covered area.
            let k = fg as f32 / bg as f32;
            let (r, g, b) = (37.0, 99.0, 235.0);
            let mix = |c: f32| (c + (255.0 - c) * k).round() as u8;
            out.push([mix(b), mix(g), mix(r), (alpha * 255.0).round() as u8]);
        }
    }
    out
}

fn bmp_frame(size: u32) -> Vec<u8> {
    let px = render(size);
    let mut v = Vec::new();
    v.extend_from_slice(&40u32.to_le_bytes());
    v.extend_from_slice(&size.to_le_bytes());
    v.extend_from_slice(&(size * 2).to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&32u16.to_le_bytes());
    v.extend_from_slice(&[0; 24]);
    for y in (0..size).rev() {
        for x in 0..size {
            v.extend_from_slice(&px[(y * size + x) as usize]);
        }
    }
    let mask_row = size.div_ceil(32) * 4;
    v.extend(std::iter::repeat_n(0u8, (mask_row * size) as usize));
    v
}

fn main() {
    let frames: Vec<Vec<u8>> = SIZES.iter().map(|&s| bmp_frame(s)).collect();
    let mut ico = Vec::new();
    ico.extend_from_slice(&[0, 0, 1, 0]);
    ico.extend_from_slice(&(SIZES.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * SIZES.len() as u32;
    for (&s, f) in SIZES.iter().zip(&frames) {
        let dim = if s >= 256 { 0 } else { s as u8 };
        ico.extend_from_slice(&[dim, dim, 0, 0]);
        ico.extend_from_slice(&1u16.to_le_bytes());
        ico.extend_from_slice(&32u16.to_le_bytes());
        ico.extend_from_slice(&(f.len() as u32).to_le_bytes());
        ico.extend_from_slice(&offset.to_le_bytes());
        offset += f.len() as u32;
    }
    for f in &frames {
        ico.extend_from_slice(f);
    }
    fs::create_dir_all("assets").unwrap();
    fs::write("assets/ycopy.ico", ico).unwrap();
    println!("assets/ycopy.ico written");
}
