//! Cross-platform Screen Region Capture + Built-in OS OCR:
//! - Windows: Captures `(x, y, w, h)` in memory via Win32 `BitBlt` and extracts text offline
//!   using built-in `Windows.Media.Ocr` (0 external dependencies, ~50ms).
//! - macOS: Interactive `screencapture -i -x` + built-in Apple `Vision` (`VNRecognizeTextRequest`).
//! - Linux: Interactive `gnome-screenshot -a` / `spectacle` + `tesseract`.

#[cfg(target_os = "windows")]
pub fn capture_rect_and_ocr(x: i32, y: i32, w: i32, h: i32) -> Result<String, String> {
    windows_ocr::capture_and_recognize_win(x, y, w, h)
}

#[cfg(not(target_os = "windows"))]
pub fn interactive_capture_and_ocr() -> Result<String, String> {
    unix_ocr::interactive_capture_and_recognize()
}

// ============================================================================
// WINDOWS: GDI BitBlt + Built-in Windows.Media.Ocr
// ============================================================================
#[cfg(target_os = "windows")]
mod windows_ocr {
    use std::mem::{size_of, zeroed};
    use std::ptr::null_mut;
    use windows::core::HSTRING;
    use windows::Globalization::Language;
    use windows::Graphics::Imaging::{BitmapAlphaMode, BitmapPixelFormat, SoftwareBitmap};
    use windows::Media::Ocr::OcrEngine;
    use windows::Security::Cryptography::CryptographicBuffer;
    use windows_sys::Win32::Graphics::Gdi::{
        BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC,
        GetDIBits, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
        SRCCOPY,
    };

    pub fn capture_and_recognize_win(x: i32, y: i32, w: i32, h: i32) -> Result<String, String> {
        if w < 8 || h < 8 {
            return Err("Vùng chọn quá nhỏ".to_string());
        }

        // Upscale tiny regions 2x so Windows OCR reads small UI/PDF fonts much more accurately!
        let bgra = capture_screen_bgra(x, y, w, h)?;
        let (final_bytes, final_w, final_h) = if w < 300 || h < 60 {
            let up = upscale_2x_bilinear(&bgra, w as usize, h as usize);
            (up, w * 2, h * 2)
        } else {
            (bgra, w, h)
        };

        let ibuffer = CryptographicBuffer::CreateFromByteArray(&final_bytes)
            .map_err(|e| format!("Lỗi tạo bộ đệm ảnh OCR: {}", e))?;

        let software_bmp = SoftwareBitmap::CreateCopyWithAlphaFromBuffer(
            &ibuffer,
            BitmapPixelFormat::Bgra8,
            final_w,
            final_h,
            BitmapAlphaMode::Ignore,
        )
        .map_err(|e| format!("Lỗi khởi tạo SoftwareBitmap: {}", e))?;

        // Prefer English OCR language pack for reading foreign text; fallback to user's installed Windows OCR language
        let en_lang = Language::CreateLanguage(&HSTRING::from("en-US")).ok();
        let engine = en_lang
            .and_then(|l| OcrEngine::TryCreateFromLanguage(&l).ok())
            .or_else(|| OcrEngine::TryCreateFromUserProfileLanguages().ok())
            .ok_or_else(|| {
                "Windows chưa bật gói ngôn ngữ OCR cơ bản (English / User Language).".to_string()
            })?;

        let ocr_result = engine
            .RecognizeAsync(&software_bmp)
            .map_err(|e| format!("Lỗi gọi Windows OCR: {}", e))?
            .get()
            .map_err(|e| format!("Lỗi nhận kết quả OCR: {}", e))?;

        let lines = ocr_result
            .Lines()
            .map_err(|e| format!("Lỗi đọc dòng OCR: {}", e))?;

        let mut extracted_lines = Vec::new();
        for line in lines {
            if let Ok(t) = line.Text() {
                let s = t.to_string_lossy();
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    extracted_lines.push(trimmed.to_string());
                }
            }
        }

        let joined = extracted_lines.join("\n");
        if joined.trim().is_empty() {
            Err("Không nhận diện được chữ trong vùng ảnh vừa chọn.".to_string())
        } else {
            Ok(joined)
        }
    }

    fn capture_screen_bgra(x: i32, y: i32, w: i32, h: i32) -> Result<Vec<u8>, String> {
        unsafe {
            let hdc_screen = GetDC(null_mut());
            if hdc_screen.is_null() {
                return Err("Không lấy được Screen DC".to_string());
            }

            let hdc_mem = CreateCompatibleDC(hdc_screen);
            if hdc_mem.is_null() {
                ReleaseDC(null_mut(), hdc_screen);
                return Err("Không tạo được Memory DC".to_string());
            }

            let hbmp = CreateCompatibleBitmap(hdc_screen, w, h);
            if hbmp.is_null() {
                DeleteDC(hdc_mem);
                ReleaseDC(null_mut(), hdc_screen);
                return Err("Không tạo được Bitmap chụp màn hình".to_string());
            }

            let old_obj = SelectObject(hdc_mem, hbmp);
            let blt_ok = BitBlt(hdc_mem, 0, 0, w, h, hdc_screen, x, y, SRCCOPY);

            let mut pixels = vec![0u8; (w as usize) * (h as usize) * 4];
            let mut bmi: BITMAPINFO = zeroed();
            bmi.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
            bmi.bmiHeader.biWidth = w;
            bmi.bmiHeader.biHeight = -h; // Negative = top-down BGRA rows
            bmi.bmiHeader.biPlanes = 1;
            bmi.bmiHeader.biBitCount = 32;
            bmi.bmiHeader.biCompression = BI_RGB;

            let lines_copied = if blt_ok != 0 {
                GetDIBits(
                    hdc_mem,
                    hbmp,
                    0,
                    h as u32,
                    pixels.as_mut_ptr() as *mut _,
                    &mut bmi,
                    DIB_RGB_COLORS,
                )
            } else {
                0
            };

            SelectObject(hdc_mem, old_obj);
            DeleteObject(hbmp);
            DeleteDC(hdc_mem);
            ReleaseDC(null_mut(), hdc_screen);

            if lines_copied == 0 {
                Err("Không thể đọc điểm ảnh màn hình".to_string())
            } else {
                Ok(pixels)
            }
        }
    }

    fn upscale_2x_bilinear(src: &[u8], w: usize, h: usize) -> Vec<u8> {
        let nw = w * 2;
        let nh = h * 2;
        let mut dst = vec![0u8; nw * nh * 4];
        for dy in 0..nh {
            let sy = (dy / 2).min(h - 1);
            let sy2 = (sy + 1).min(h - 1);
            let wy = (dy & 1) as u32;
            for dx in 0..nw {
                let sx = (dx / 2).min(w - 1);
                let sx2 = (sx + 1).min(w - 1);
                let wx = (dx & 1) as u32;

                let p00 = (sy * w + sx) * 4;
                let p10 = (sy * w + sx2) * 4;
                let p01 = (sy2 * w + sx) * 4;
                let p11 = (sy2 * w + sx2) * 4;
                let out = (dy * nw + dx) * 4;

                for c in 0..4 {
                    let top = (src[p00 + c] as u32) * (2 - wx) + (src[p10 + c] as u32) * wx;
                    let bot = (src[p01 + c] as u32) * (2 - wx) + (src[p11 + c] as u32) * wx;
                    dst[out + c] = ((top * (2 - wy) + bot * wy) / 4) as u8;
                }
            }
        }
        dst
    }
}

// ============================================================================
// MACOS (Built-in Vision Framework) & LINUX (Tesseract)
// ============================================================================
#[cfg(not(target_os = "windows"))]
mod unix_ocr {
    use std::fs;
    use std::process::Command;

    pub fn interactive_capture_and_recognize() -> Result<String, String> {
        let tmp_img = std::env::temp_dir().join("qtvn_ocr_capture.png");
        let _ = fs::remove_file(&tmp_img);
        let img_str = tmp_img.to_string_lossy().to_string();

        #[cfg(target_os = "macos")]
        {
            // Interactive native macOS region snip
            let status = Command::new("screencapture")
                .args(["-i", "-x", &img_str])
                .status()
                .map_err(|e| e.to_string())?;

            if !status.success() || !tmp_img.exists() {
                return Err(String::new()); // User cancelled with ESC
            }

            // Use macOS built-in Vision framework via JXA (zero external dependencies)
            let jxa = format!(
                r#"ObjC.import('Foundation'); ObjC.import('Vision'); ObjC.import('AppKit');
                const url = $.NSURL.fileURLWithPath('{}');
                const img = $.NSImage.alloc.initWithContentsOfURL(url);
                let rect = $.NSMakeRect(0, 0, img.size.width, img.size.height);
                const cgImg = img.CGImageForProposedRectContextHints(Ref(rect), null, null);
                const req = $.VNRecognizeTextRequest.alloc.init;
                req.recognitionLevel = $.VNRequestTextRecognitionLevelAccurate;
                const handler = $.VNImageRequestHandler.alloc.initWithCGImageOptions(cgImg, $({{}}));
                handler.performRequestsError($.NSArray.arrayWithObject(req), null);
                const obs = req.results;
                const out = [];
                for (let i = 0; i < obs.count; i++) {{
                    out.push(ObjC.unwrap(obs.objectAtIndex(i).topCandidates(1).objectAtIndex(0).string));
                }}
                out.join('\n');"#,
                img_str.replace('\'', "\\'")
            );

            let output = Command::new("osascript")
                .args(["-l", "JavaScript", "-e", &jxa])
                .output()
                .map_err(|e| e.to_string())?;
            let _ = fs::remove_file(&tmp_img);

            let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if text.is_empty() {
                Err("Không nhận diện được chữ trong vùng vừa chụp.".to_string())
            } else {
                Ok(text)
            }
        }

        #[cfg(target_os = "linux")]
        {
            let captured = Command::new("gnome-screenshot")
                .args(["-a", "-f", &img_str])
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
                || Command::new("spectacle")
                    .args(["-r", "-b", "-n", "-o", &img_str])
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false);

            if !captured || !tmp_img.exists() {
                return Err(String::new());
            }

            let out = Command::new("tesseract")
                .args([&img_str, "stdout", "-l", "eng"])
                .output()
                .map_err(|_| {
                    "Cần cài gói `tesseract-ocr` trên Linux để đọc chữ từ ảnh.".to_string()
                })?;
            let _ = fs::remove_file(&tmp_img);

            let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if text.is_empty() {
                Err("Không nhận diện được chữ trong vùng vừa chụp.".to_string())
            } else {
                Ok(text)
            }
        }
    }
}
