# 🌐 QuickTranslateVN

**Công cụ dịch văn bản & OCR tức thì trên toàn hệ điều hành Windows (System-wide Instant Translation Tool)** — mang trải nghiệm của *Google Translate Extension*, *DeepL Desktop* và *Trayslate* ra mọi ứng dụng Windows (Word, Excel, PowerPoint, PDF Reader, Chrome, Edge, VS Code, Slack, Notepad...).

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Platform: Windows 10/11](https://img.shields.io/badge/Platform-Windows%2010%2F11-0078D6.svg)](#)
[![.NET 8 WPF](https://img.shields.io/badge/.NET-8.0%20WPF-512BD4.svg)](#)

---

## ✨ Điểm Nổi Bật

- **5 phương thức kích hoạt linh hoạt:**
  1. **Mouse Mode (`[文A]`):** Kéo chuột bôi đen hoặc nháy đúp chọn từ → nút dịch nổi cạnh trỏ chuột (chỉ hiện trên vùng chữ `IDC_IBEAM`, không làm phiền khi kéo file hay thanh cuộn).
  2. **`Alt + Q`:** Dịch ngay lập tức đoạn văn bản đang bôi đen.
  3. **`Ctrl + C + C` (kiểu *DeepL Desktop*):** Nhấn đúp `Ctrl + C` hai lần liên tiếp để bung Popup dịch.
  4. **`Ctrl + Shift + T` (kiểu *QuickTranslate / TrayTranslator*):** Dịch vùng đang chọn hoặc mở ngay khung tự gõ/dán văn bản nhanh.
  5. **`Alt + S` (Screen OCR):** Khoanh vùng bất kỳ trên màn hình (PDF scan ảnh, PDF khóa copy, slide, video) để nhận diện chữ **100% Offline** qua `Windows.Media.Ocr` và dịch tức thì.
- **Thay thế văn bản trực tiếp tại chỗ (`⏎ Thay thế` — kiểu *Trayslate & DeepL*):** Đang soạn thảo tiếng Việt trong Email/Slack/Word/VS Code → bôi đen dịch sang tiếng Anh → bấm **`⏎ Thay thế`** để điền thẳng bản dịch vào đúng vị trí đang gõ mà không mất focus.
- **Đa nguồn dịch & Hỗ trợ 100% Offline AI:**
  - **Google Translate (Miễn phí - Mặc định):** Kèm phiên âm `/phonetic/` và từ điển phân theo từ loại (*danh từ, động từ, tính từ*).
  - **Microsoft / Bing Translator (Miễn phí):** Không cần API key, dịch mượt các đoạn văn dài.
  - **DeepL API:** Hỗ trợ cả khóa Free (`:fx`) và Pro.
  - **AI / LLM (Chuẩn OpenAI-compatible):** Cắm trực tiếp **Ollama chạy nội bộ (`localhost:11434` — bảo mật Offline 100%)**, OpenAI, DeepSeek, Groq...
- **Dịch ngược kiểm chứng (`↩ Dịch ngược` — kiểu *OpenTranslate*):** Kiểm tra lại sắc thái nghĩa của bản dịch bằng cách dịch ngược về ngôn ngữ gốc.
- **Tối ưu sâu cho Windows & Tiết kiệm RAM:**
  - Cửa sổ Fluent Dark `WS_EX_NOACTIVATE` **không bao giờ cướp focus** của ứng dụng đang làm việc; hỗ trợ màn hình đa tỷ lệ phân giải (`PerMonitorV2` DPI).
  - Thác thu thập văn bản thông minh: **UI Automation (`TextPattern` + Excel `ValuePattern`) → Terminal Safety Guard → Safe Clipboard** (khôi phục đa định dạng kèm cờ chặn ghi rác vào `Win + V`).
  - Luồng STA đơn + cơ chế tự động thu gọn bộ nhớ nền (`MemoryOptimizer`) giữ mức RAM System Tray cực nhẹ.

---

## ⌨️ Bảng Phím Tắt & Thao Tác

| Thao tác / Phím tắt | Chức năng |
|---|---|
| **Kéo bôi đen → bấm `[文A]`** | Hiện Popup dịch ngay cạnh trỏ chuột |
| **`Alt + Q`** | Dịch tức thì văn bản đang chọn ở bất kỳ ứng dụng nào |
| **`Ctrl + C` (nhấn đúp 2 lần)** | Copy và hiện ngay bản dịch (có thể bật/tắt dưới Tray) |
| **`Ctrl + Shift + T`** | Mở khung tự nhập văn bản nhanh (hoặc dịch vùng đang chọn) |
| **`Alt + S`** | Khoanh vùng màn hình quét OCR (`Windows.Media.Ocr`) + Dịch |
| **`ESC` / Click ra ngoài** | Đóng nhanh Popup (trừ khi đang bấm `📌 Ghim`) |

---

## 🏗️ Kiến Trúc Hệ Thống

```text
QuickTranslateVN/
├── Interop/
│   └── NativeMethods.cs           # Win32 P/Invoke (Hooks, Hotkeys, SendInput, DPI, Clipboard flags)
├── Services/
│   ├── AppSettings.cs             # Cấu hình JSON + Mã hóa khóa API bằng Windows DPAPI
│   ├── GlobalInputManager.cs      # Quản lý Hotkeys, Double Ctrl+C & IBeam Mouse Selection
│   ├── TextCaptureService.cs      # Single STA Worker: UIA (Word/Excel) -> Safe Clipboard + In-place Replace
│   ├── TextCleaner.cs             # Nối dòng gãy PDF, chuẩn hóa Unicode ligatures & nhận diện Tiếng Việt
│   ├── TranslationService.cs      # Google GTX, Microsoft/Bing, DeepL, AI/Ollama + TTS + Back-translation
│   ├── ScreenOcrService.cs        # Nhận diện chữ trên ảnh Offline bằng Windows.Media.Ocr
│   ├── HistoryService.cs          # Lưu lịch sử tra cứu cục bộ (hỗ trợ chế độ Ẩn danh)
│   └── MemoryOptimizer.cs         # Tự động thu gọn Working Set RAM khi đóng Popup
├── UI/
│   ├── FloatingBadgeWindow.xaml   # Nút nổi [文A] non-activating cạnh trỏ chuột
│   ├── TranslationPopupWindow.xaml# Cửa sổ dịch Fluent UI (Đảo chiều, Thay thế, Dịch ngược, TTS, Từ điển)
│   ├── OcrOverlayWindow.xaml      # Màn hình khoanh vùng chụp OCR đa màn hình
│   ├── SettingsWindow.xaml        # Hộp thoại Cài đặt, AI Presets & Lịch sử dịch
│   └── WindowPositionHelper.cs    # Tính toán Per-Monitor DPI & chống tràn mép màn hình
└── App.xaml.cs                    # Quản lý vòng đời ứng dụng & System Tray
```

---

## 🚀 Hướng Dẫn Cài Đặt & Biên Dịch

### Yêu cầu
- Hệ điều hành: **Windows 10** (build 19041+) hoặc **Windows 11**.
- [.NET 8.0 Desktop Runtime](https://dotnet.microsoft.com/en-us/download/dotnet/8.0) (hoặc .NET 8 SDK nếu tự biên dịch).

### Biên dịch ra file `.exe` đơn lẻ (Single-File Release)
```powershell
git clone https://github.com/<your-username>/QuickTranslateVN.git
cd QuickTranslateVN
dotnet publish QuickTranslateVN.csproj -c Release -r win-x64 --self-contained false -p:PublishSingleFile=true -o ./dist
```
Sau khi biên dịch, chạy trực tiếp `./dist/QuickTranslateVN.exe` — biểu tượng **`VN`** màu xanh sẽ xuất hiện ở khay hệ thống (System Tray).

---

## 🔒 Bảo Mật & Quyền Riêng Tư

Xem chi tiết tại [PRIVACY.md](PRIVACY.md):
- **0% Telemetry / Analytics** — Không máy chủ trung gian.
- **Không Keylogging** — Không đọc văn bản cho tới khi bạn chủ động kích hoạt dịch.
- **Mã hóa khóa API bằng Windows DPAPI** & hỗ trợ chạy **100% Offline qua Ollama**.

---

## 📄 Giấy Phép (License)

Phát hành dưới giấy phép [MIT License](LICENSE).
