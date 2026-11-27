fn main() {
    #[cfg(target_os = "windows")]
    windows_res::embed();
}

#[cfg(target_os = "windows")]
mod windows_res {
    use std::env;
    use std::fs;
    use std::path::PathBuf;

    pub fn embed() {
        let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
        let ico_path = out_dir.join("QuickTranslateVN.ico");
        let ico_bytes = generate_64x64_ico();
        fs::write(&ico_path, ico_bytes).expect("failed to write generated .ico");

        let mut res = winresource::WindowsResource::new();
        res.set_icon(ico_path.to_str().unwrap());
        res.set("ProductName", "QuickTranslateVN");
        res.set(
            "FileDescription",
            "QuickTranslateVN — System-wide Instant Text & Screen OCR Translator",
        );
        res.set("CompanyName", "Trunguyen104 Open Source");
        res.set(
            "LegalCopyright",
            "Copyright (c) 2026 Trunguyen104 — MIT License",
        );
        res.set("OriginalFilename", "QuickTranslateVN.exe");
        let _ = res.compile();
    }

    /// Generates a valid 64x64 32-bit BGRA uncompressed BMP-in-ICO icon in pure Rust.
    fn generate_64x64_ico() -> Vec<u8> {
        const W: usize = 64;
        const H: usize = 64;

        const GLYPH_V: [&str; 10] = [
            "##...##", "##...##", "##...##", ".##.##.", ".##.##.", ".##.##.", "..###..", "..###..",
            "..###..", "...#...",
        ];
        const GLYPH_N: [&str; 10] = [
            "##...##", "###..##", "####.##", "##.####", "##..###", "##...##", "##...##", "##...##",
            "##...##", "##...##",
        ];

        // BGRA pixel order for BMP
        let blue = [235u8, 99u8, 37u8, 255u8];
        let white = [255u8, 255u8, 255u8, 255u8];
        let mut bgra = vec![0u8; W * H * 4];

        for y in 0..H {
            for x in 0..W {
                let fx = x as f32 + 0.5;
                let fy = y as f32 + 0.5;
                let qx = (fx - 32.0).abs() - (27.0 - 14.0);
                let qy = (fy - 32.0).abs() - (27.0 - 14.0);
                let dist = (qx.max(0.0).hypot(qy.max(0.0))) + qx.max(qy).min(0.0) - 14.0;

                let idx = (y * W + x) * 4;
                if dist <= 0.8 {
                    let alpha = (0.8 - dist).clamp(0.0, 1.0);
                    if dist >= -4.2 {
                        let t = ((dist + 4.2) / 1.2).clamp(0.0, 1.0);
                        bgra[idx] = lerp(white[0], blue[0], t);
                        bgra[idx + 1] = lerp(white[1], blue[1], t);
                        bgra[idx + 2] = lerp(white[2], blue[2], t);
                        bgra[idx + 3] = (255.0 * alpha) as u8;
                    } else {
                        bgra[idx..idx + 4].copy_from_slice(&white);
                    }
                }
            }
        }

        // Stamp 2x scaled "VN" glyphs
        stamp_2x(&mut bgra, W, 16, 22, &GLYPH_V, blue);
        stamp_2x(&mut bgra, W, 34, 22, &GLYPH_N, blue);

        // ICO BMP rows are stored bottom-up
        let mut bmp_pixels = Vec::with_capacity(W * H * 4);
        for y in (0..H).rev() {
            let row_start = y * W * 4;
            bmp_pixels.extend_from_slice(&bgra[row_start..row_start + W * 4]);
        }

        // 1-bit AND mask (8 bytes per row = 512 bytes, all 0 since alpha channel is in 32-bit BGRA)
        let and_mask = vec![0u8; (W / 8) * H];
        let dib_size = 40 + bmp_pixels.len() + and_mask.len();

        let mut ico = Vec::with_capacity(22 + dib_size);
        // ICONDIR (6 bytes)
        ico.extend_from_slice(&0u16.to_le_bytes()); // Reserved
        ico.extend_from_slice(&1u16.to_le_bytes()); // Type = 1 (ICO)
        ico.extend_from_slice(&1u16.to_le_bytes()); // Count = 1

        // ICONDIRENTRY (16 bytes)
        ico.push(W as u8);
        ico.push(H as u8);
        ico.push(0); // ColorCount
        ico.push(0); // Reserved
        ico.extend_from_slice(&1u16.to_le_bytes()); // Planes
        ico.extend_from_slice(&32u16.to_le_bytes()); // BitCount
        ico.extend_from_slice(&(dib_size as u32).to_le_bytes()); // BytesInRes
        ico.extend_from_slice(&22u32.to_le_bytes()); // ImageOffset

        // BITMAPINFOHEADER (40 bytes) — height is doubled (XOR + AND masks) per ICO spec
        ico.extend_from_slice(&40u32.to_le_bytes());
        ico.extend_from_slice(&(W as i32).to_le_bytes());
        ico.extend_from_slice(&((H * 2) as i32).to_le_bytes());
        ico.extend_from_slice(&1u16.to_le_bytes()); // Planes
        ico.extend_from_slice(&32u16.to_le_bytes()); // BitCount
        ico.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
        ico.extend_from_slice(&(bmp_pixels.len() as u32).to_le_bytes());
        ico.extend_from_slice(&0i32.to_le_bytes());
        ico.extend_from_slice(&0i32.to_le_bytes());
        ico.extend_from_slice(&0u32.to_le_bytes());
        ico.extend_from_slice(&0u32.to_le_bytes());

        ico.extend_from_slice(&bmp_pixels);
        ico.extend_from_slice(&and_mask);
        ico
    }

    fn lerp(a: u8, b: u8, t: f32) -> u8 {
        ((a as f32) * (1.0 - t) + (b as f32) * t).round() as u8
    }

    fn stamp_2x(
        bgra: &mut [u8],
        stride: usize,
        ox: usize,
        oy: usize,
        rows: &[&str],
        color: [u8; 4],
    ) {
        for (ry, row) in rows.iter().enumerate() {
            for (rx, ch) in row.chars().enumerate() {
                if ch == '#' {
                    for dy in 0..2 {
                        for dx in 0..2 {
                            let idx = ((oy + ry * 2 + dy) * stride + (ox + rx * 2 + dx)) * 4;
                            bgra[idx..idx + 4].copy_from_slice(&color);
                        }
                    }
                }
            }
        }
    }
}
