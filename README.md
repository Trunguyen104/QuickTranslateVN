# 🌐 QuickTranslateVN (Rust Cross-Platform)

**Công cụ dịch nhanh văn bản đang chọn & chụp màn hình OCR trên toàn hệ thống — viết 100% bằng Rust, siêu nhẹ, giao diện Trắng Tối Giản, tương thích mọi nền tảng (Windows • macOS • Linux).**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Language: Rust](https://img.shields.io/badge/Built%20with-Rust%20%F0%9F%A6%80-orange.svg)](#)
[![Platform: Windows | macOS | Linux](https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20%7C%20Linux-2563EB.svg)](#)

---

## ✨ 4 Cách Dịch Nhanh Toàn Hệ Thống

1. **Chụp vùng màn hình đọc chữ OCR & Dịch (`Alt + S` / `Option + S`):**
   - Nhấn **`Alt + S`** → Kéo khung khoanh vùng bất kỳ trên màn hình (ảnh chụp, slide, video, PDF scan khóa copy) → Ứng dụng dùng ngay **nhân OCR tích hợp sẵn của hệ điều hành** (`Windows.Media.Ocr` trên Windows / `Apple Vision` trên macOS — **0 MB dung lượng phụ, chạy Offline**) để đọc chữ và hiện ngay Popup dịch!
2. **Kéo chuột bôi đen chữ → bấm nút `[文A]`** nổi cạnh trỏ chuột *(chỉ hiện trên vùng chữ `IDC_IBEAM`)*.
3. **Bôi đen chữ → nhấn `Alt + Q`** (`Option + Q` trên macOS).
4. **Bôi đen chữ → nhấn đúp `Ctrl + C + C`** (`Cmd + C + C` trên macOS — kiểu DeepL Desktop).

*(Bạn có thể tự đổi phím tắt `Alt + S` và `Alt + Q` sang bất kỳ tổ hợp phím nào khác ngay trong menu chuột phải dưới System Tray).*

---

## 📥 Tải Về & Hướng Dẫn Mở Nhanh

Tải bản đóng gói sẵn cho **Windows (`.zip`)**, **macOS (`.tar.gz`)**, hoặc **Linux (`.tar.gz`)** kèm bảng kiểm chứng `SHA256SUMS.txt` tại trang **[GitHub Releases](https://github.com/Trunguyen104/QuickTranslateVN/releases)**.

> **💡 Lưu ý nhỏ trên Windows (SmartScreen):** Vì đây là dự án mã nguồn mở miễn phí chưa mua chứng chỉ ký số thương mại trả phí, trong lần mở đầu tiên sau khi giải nén `QuickTranslateVN-windows-x64.zip`, nếu Windows hiện bảng xanh *"Windows protected your PC"*, bạn chỉ cần bấm **More info (Thông tin thêm)** → **Run anyway (Vẫn chạy)**.

### Tự biên dịch từ mã nguồn bằng Cargo
```bash
git clone https://github.com/Trunguyen104/QuickTranslateVN.git
cd QuickTranslateVN
cargo build --release
```

---

## 🌍 Trang Chủ Giới Thiệu (GitHub Pages)

Trải nghiệm bản Demo tương tác trực tiếp tại: **https://trunguyen104.github.io/QuickTranslateVN/**

---

## 🔒 Quyền Riêng Tư & Giấy Phép

- **100% Offline OCR:** Ảnh chụp khoanh vùng màn hình được nhận diện chữ ngay trên máy bởi hệ điều hành, không bao giờ tải ảnh lên mạng.
- Xem chi tiết tại [PRIVACY.md](PRIVACY.md) • Phát hành dưới giấy phép [MIT License](LICENSE).
