# 🌐 QuickTranslateVN (Rust Cross-Platform)

**Công cụ dịch nhanh văn bản đang chọn trên toàn hệ thống — viết 100% bằng Rust, siêu nhẹ, giao diện Trắng Tối Giản, tương thích mọi nền tảng (Windows • macOS • Linux).**

Mang trải nghiệm gọn nhẹ của *Google Translate Extension* và *DeepL Desktop* ra mọi ứng dụng trên máy tính (Word, Excel, PDF Reader, Chrome, Safari, Firefox, VS Code, Slack, Notepad...).

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Language: Rust](https://img.shields.io/badge/Built%20with-Rust%20%F0%9F%A6%80-orange.svg)](#)
[![Platform: Windows | macOS | Linux](https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20%7C%20Linux-2563EB.svg)](#)

---

## ✨ Điểm Nổi Bật

- **100% Native Rust (Không cần cài .NET hay Node.js Runtime):** Đóng gói thành một file chạy mã máy duy nhất, chạy mượt trên **Windows 10/11**, **macOS (Apple Silicon & Intel)** và **Linux (X11 & Wayland)**.
- **3 Cách Kích Hoạt Tức Thì:**
  1. **Kéo chuột bôi đen chữ → bấm nút `[文A]`** nổi cạnh trỏ chuột.
  2. **Bôi đen chữ → nhấn `Alt + Q`** (`Option + Q` trên macOS).
  3. **Bôi đen chữ → nhấn đúp `Ctrl + C + C`** (`Cmd + C + C` trên macOS — kiểu DeepL Desktop).
- **Thanh Popup Trắng Tối Giản (Clean White UI):**
  - **`EN → VI ⇄`:** Tự động nhận diện chiều dịch Anh ↔ Việt (bấm để đảo chiều).
  - **Nghe phát âm**, **Sao chép**, **Thay thế trực tiếp vào ô đang soạn (`⏎`)**, và **Ghim cửa sổ**.
  - Tự động hiển thị phiên âm `/.../` và từ điển theo từ loại khi bôi đen từ lẻ.
- **Tối Ưu Sâu Theo Từng Hệ Điều Hành:**
  - **Windows:** Cửa sổ `WS_EX_NOACTIVATE` không cướp focus, nhận diện con trỏ `IDC_IBEAM`, khôi phục Clipboard kèm cờ chống ghi rác `Win + V`.
  - **Linux:** Đọc trực tiếp bộ nhớ đệm **Primary Selection** (X11/Wayland) không cần giả lập phím, kèm fallback `xdotool`/`wtype`.
  - **macOS:** Chạy gọn trên thanh **Menu Bar** (`ActivationPolicy::Accessory`, không chiếm chỗ dưới Dock), lưu cấu hình chuẩn LaunchAgents.

---

## 🚀 Tải Về & Biên Dịch Từ Mã Nguồn

### Cách 1: Tải bản biên dịch sẵn
Tải gói phù hợp với hệ điều hành của bạn (`Windows .exe`, `macOS .tar.gz`, `Linux .tar.gz`) tại trang **[GitHub Releases](https://github.com/Trunguyen104/QuickTranslateVN/releases)**.

### Cách 2: Tự biên dịch bằng Cargo (Rust)

*(Trên Ubuntu/Debian Linux, cài gói thư viện WebView hệ thống trước khi build: `sudo apt install pkg-config libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev libxdo-dev libx11-dev libxi-dev libxtst-dev`)*

```bash
git clone https://github.com/Trunguyen104/QuickTranslateVN.git
cd QuickTranslateVN
cargo build --release
```
File thực thi sau khi biên dịch nằm tại `target/release/QuickTranslateVN` (hoặc `QuickTranslateVN.exe` trên Windows).

---

## 🌍 Trang Chủ Giới Thiệu (GitHub Pages)

Xem bản Demo tương tác trực tiếp trên trình duyệt tại: **https://trunguyen104.github.io/QuickTranslateVN/** (mã nguồn nằm tại thư mục [`docs/`](docs/index.html)).

---

## 🔒 Quyền Riêng Tư & Giấy Phép

- Xem chi tiết chính sách bảo mật tại [PRIVACY.md](PRIVACY.md) (0% Telemetry, không lưu lịch sử, không đọc lén bàn phím).
- Phát hành dưới giấy phép [MIT License](LICENSE).
