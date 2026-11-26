# Chính Sách Bảo Mật & Quyền Riêng Tư (Privacy & Security Policy)

**QuickTranslateVN** được thiết kế theo nguyên tắc **Privacy-First (Ưu tiên quyền riêng tư tuyệt đối)**:

## 1. Không Thu Thập Dữ Liệu Ngầm (Zero Telemetry)
- Ứng dụng **không** chứa bất kỳ SDK theo dõi, phân tích hành vi (analytics), quảng cáo hay máy chủ trung gian nào.
- Không thu thập thông tin cá nhân, tên máy tính hay danh sách ứng dụng đang mở.

## 2. Cơ Chế Hook Bàn Phím & Chuột Minh Bạch (No Keylogging)
- Global Keyboard Hook (`WH_KEYBOARD_LL`) **chỉ kiểm tra phím `ESC`** (để đóng popup) và **nhịp nhấn đúp `Ctrl + C + C`**. Không ghi nhận, lưu trữ hay truyền đi bất kỳ phím gõ văn bản/mật khẩu nào.
- Khi kéo chuột bôi đen văn bản, ứng dụng chỉ kiểm tra hình dạng con trỏ (`IDC_IBEAM`) để hiển thị biểu tượng `[文A]` — **văn bản hoàn toàn chưa bị đọc** cho đến khi người dùng chủ động bấm nút dịch hoặc nhấn phím tắt.

## 3. Bảo Toàn Clipboard & Lịch Sử `Win + V`
- Ưu tiên đọc văn bản qua **Windows UI Automation (`TextPattern` / `ValuePattern`)** mà không chạm vào Clipboard.
- Khi bắt buộc phải dùng *Clipboard Fallback*, ứng dụng sao lưu các định dạng dữ liệu hiện có của người dùng và khôi phục lại ngay sau vài chục mili-giây kèm cờ hệ thống `ExcludeClipboardContentFromMonitorProcessing` và `CanIncludeInClipboardHistory = 0` để tránh ghi rác lên Windows Clipboard History (`Win + V`) và Cloud Clipboard.

## 4. Lưu Trữ Cục Bộ & Mã Hóa Khóa API (DPAPI)
- Cấu hình lưu cục bộ tại `%LocalAppData%\QuickTranslateVN\settings.json`.
- Mọi khóa API cá nhân (DeepL, OpenAI, DeepSeek...) đều được mã hóa bằng **Windows DPAPI (`ProtectedData` — `DataProtectionScope.CurrentUser`)**, chỉ tài khoản Windows của chính bạn mới giải mã được, không bao giờ lưu dạng văn bản thuần (plain text) trên ổ cứng.
- Người dùng có thể tắt lưu lịch sử dịch bất kỳ lúc nào để chạy ở **Chế độ Ẩn danh (Incognito)**.

## 5. Dữ Liệu Gửi Tới Nhà Cung Cấp Dịch Thuật & Chế Độ 100% Offline
- Ở chế độ trực tuyến (Google / Microsoft / DeepL / Cloud AI), đoạn văn bản bạn yêu cầu dịch được gửi trực tiếp qua kết nối HTTPS từ máy bạn tới API của nhà cung cấp tương ứng.
- Quá trình nhận diện chữ trên ảnh (**OCR `Alt + S`**) chạy **100% Offline** bằng nhân `Windows.Media.Ocr` của hệ điều hành — ảnh chụp màn hình không bao giờ rời khỏi máy tính.
- Đối với tài liệu mật nội bộ doanh nghiệp, bạn có thể chọn nguồn **`AI / LLM`** trỏ về **`http://localhost:11434/v1/chat/completions` (Ollama Local)** để toàn bộ quy trình từ bắt chữ, OCR đến dịch thuật diễn ra **100% Offline trên máy tính của bạn**.
