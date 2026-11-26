# 🌐 QuickTranslateVN

**Công cụ dịch nhanh văn bản đang chọn trên toàn hệ điều hành Windows — Siêu nhẹ, Zero-Config, 0 thư viện ngoài.**

Mang trải nghiệm gọn nhẹ của *Google Translate Extension* và *DeepL Ctrl+C+C* ra mọi ứng dụng Windows (Word, Excel, PowerPoint, PDF Reader, Chrome, Edge, VS Code, Notepad...).

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![.NET 8 WPF](https://img.shields.io/badge/.NET-8.0%20Zero--Dependency-512BD4.svg)](#)

---

## ✨ Sử Dụng Ngay (Không Cần Cài Đặt Phức Tạp)

1. **Kéo chuột bôi đen chữ → bấm nút `[文A]`** hiện cạnh chuột *(chỉ hiện khi bôi đen chữ, tự ẩn nếu không bấm)*.
2. **Hoặc bôi đen chữ → nhấn `Alt + Q`** để hiện thẳng Popup dịch tức thì.
3. **Hoặc bôi đen chữ → nhấn đúp `Ctrl + C + C`** (kiểu DeepL Desktop).

### Chức năng ngay trên thanh Popup gọn nhẹ:
- **`EN → VI ⇄`:** Tự động nhận diện chiều dịch Anh ↔ Việt (bấm để lật chiều).
- **`🔊`:** Nghe phát âm câu gốc.
- **`⏎`:** Thay thế trực tiếp đoạn đang chọn trong ứng dụng gốc bằng bản dịch.
- **`📋`:** Sao chép bản dịch.
- **`📌` / `✕`:** Ghim cửa sổ hoặc đóng nhanh (`ESC` / bấm chuột ra ngoài tự đóng).
- **Từ điển & Phiên âm:** Khi chọn từ vựng lẻ, tự động hiện thêm phiên âm `/.../` và nghĩa theo từ loại.

### Tùy chọn tối giản dưới System Tray (góc phải Taskbar):
Nhấp chuột phải vào biểu tượng **`VN`** để bật/tắt nhanh:
- `☑ Hiện nút [文A] khi bôi đen chuột`
- `☑ Dịch khi nhấn đúp Ctrl + C + C`
- `☑ Khởi động cùng Windows`

---

## ⚡ Tại Sao Siêu Nhẹ?

- **0 gói NuGet ngoài (Zero External Dependencies):** Viết hoàn toàn trên `.NET 8` & Win32 API gốc, dung lượng file `.exe` chỉ **~180 KB**.
- **Không chiếm Focus (`WS_EX_NOACTIVATE`):** Không làm mất vùng đang chọn hay gián đoạn việc gõ phím trong ứng dụng hiện tại.
- **Bảo vệ Clipboard & `Win + V`:** Ưu tiên đọc qua **Windows UI Automation** (Word, Excel, Notepad) trước; khi fallback sang Clipboard sẽ khôi phục nguyên vẹn dữ liệu cũ và gắn cờ chặn ghi rác lịch sử `Win + V`.
- **Tự động thu gọn RAM nền:** Mức Working Set khi chạy ngầm dưới System Tray chỉ vài MB.

---

## 🚀 Biên Dịch Từ Mã Nguồn

```powershell
dotnet publish QuickTranslateVN.csproj -c Release -r win-x64 --self-contained false -p:PublishSingleFile=true -o ./dist
```
Chạy trực tiếp `./dist/QuickTranslateVN.exe` (yêu cầu máy có sẵn [.NET 8 Desktop Runtime](https://dotnet.microsoft.com/en-us/download/dotnet/8.0)).

---

## 🌍 Trang Chủ Giới Thiệu (GitHub Pages)

Thư mục [`docs/`](docs/index.html) chứa trang Landing Page giới thiệu công cụ kèm **Demo tương tác bôi đen dịch trực tiếp ngay trên trình duyệt** (hỗ trợ song ngữ `VI / EN`).

Để bật trang web sau khi push lên GitHub:
1. Vào **Settings → Pages** trên repository GitHub của bạn.
2. Tại mục **Build and deployment → Source**, chọn **GitHub Actions** (hoặc chọn **Deploy from a branch** → nhánh `main` → thư mục `/docs`).
3. Website sẽ tự động xuất bản tại `https://<username>.github.io/<repo-name>/`.

---

## 🔒 Quyền Riêng Tư & Giấy Phép

- Xem chi tiết chính sách bảo mật tại [PRIVACY.md](PRIVACY.md) (0% Telemetry, không lưu lịch sử, không đọc lén bàn phím).
- Phát hành dưới giấy phép [MIT License](LICENSE).
