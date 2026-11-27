use tray_icon::Icon;

/// Generates a crisp 32x32 RGBA Clean White rounded-square icon with a `#2563EB` border
/// and bold `"VN"` letters — zero external image library dependencies!
pub fn build_tray_icon() -> Icon {
    const W: usize = 32;
    const H: usize = 32;
    let mut rgba = vec![0u8; W * H * 4];

    // 5x9 crisp pixel bitmap for 'V' and 'N' (scaled 1.5x / crisp stroke)
    const GLYPH_V: [&str; 10] = [
        "##...##",
        "##...##",
        "##...##",
        ".##.##.",
        ".##.##.",
        ".##.##.",
        "..###..",
        "..###..",
        "..###..",
        "...#...",
    ];

    const GLYPH_N: [&str; 10] = [
        "##...##",
        "###..##",
        "####.##",
        "##.####",
        "##..###",
        "##...##",
        "##...##",
        "##...##",
        "##...##",
        "##...##",
    ];

    let blue = [37u8, 99u8, 235u8, 255u8];
    let white = [255u8, 255u8, 255u8, 255u8];

    for y in 0..H {
        for x in 0..W {
            let fx = x as f32 + 0.5;
            let fy = y as f32 + 0.5;

            // Signed distance to rounded box centered at (16, 16), half-size 13.5, radius 7.0
            let qx = (fx - 16.0).abs() - (13.5 - 7.0);
            let qy = (fy - 16.0).abs() - (13.5 - 7.0);
            let dist = (qx.max(0.0).hypot(qy.max(0.0))) + qx.max(qy).min(0.0) - 7.0;

            let idx = (y * W + x) * 4;
            if dist <= 0.6 {
                let outer_alpha = (0.6 - dist).clamp(0.0, 1.0);
                // 2.1px border ring
                if dist >= -2.1 {
                    let t = ((dist + 2.1) / 1.0).clamp(0.0, 1.0);
                    rgba[idx] = lerp(white[0], blue[0], t);
                    rgba[idx + 1] = lerp(white[1], blue[1], t);
                    rgba[idx + 2] = lerp(white[2], blue[2], t);
                    rgba[idx + 3] = (255.0 * outer_alpha) as u8;
                } else {
                    rgba[idx..idx + 4].copy_from_slice(&white);
                }
            }
        }
    }

    // Stamp crisp "VN" centered at y=11..21
    draw_glyph(&mut rgba, W, 8, 11, &GLYPH_V, blue);
    draw_glyph(&mut rgba, W, 17, 11, &GLYPH_N, blue);

    Icon::from_rgba(rgba, W as u32, H as u32).expect("valid 32x32 RGBA tray icon")
}

fn lerp(a: u8, b: u8, t: f32) -> u8 {
    ((a as f32) * (1.0 - t) + (b as f32) * t).round() as u8
}

fn draw_glyph(rgba: &mut [u8], stride: usize, ox: usize, oy: usize, rows: &[&str], color: [u8; 4]) {
    for (ry, row) in rows.iter().enumerate() {
        for (rx, ch) in row.chars().enumerate() {
            if ch == '#' {
                let idx = ((oy + ry) * stride + (ox + rx)) * 4;
                rgba[idx..idx + 4].copy_from_slice(&color);
            }
        }
    }
}
