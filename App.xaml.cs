using System.Drawing;
using System.Drawing.Drawing2D;
using System.Windows;
using System.Windows.Forms;
using QuickTranslateVN.Interop;
using QuickTranslateVN.Services;
using QuickTranslateVN.UI;

namespace QuickTranslateVN;

public partial class App : System.Windows.Application
{
    private Mutex? _singleInstanceMutex;
    private NotifyIcon? _trayIcon;
    private Icon? _generatedIcon;

    private AppSettings _settings = null!;
    private HistoryService _historyService = null!;
    private GlobalInputManager _inputManager = null!;
    private TextCaptureService _captureService = null!;
    private TranslationService _translationService = null!;
    private ScreenOcrService _ocrService = null!;

    private FloatingBadgeWindow _badgeWindow = null!;
    private TranslationPopupWindow _popupWindow = null!;
    private SettingsWindow? _settingsWindow;

    private ToolStripMenuItem _badgeMenuToggle = null!;
    private ToolStripMenuItem _dblCopyMenuToggle = null!;
    private ToolStripMenuItem _pdfCleanMenuToggle = null!;
    private ToolStripMenuItem _startupMenuToggle = null!;

    private bool _isBusyCapturing;
    private bool _isOcrOpen;

    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);

        _singleInstanceMutex = new Mutex(true, "Local\\QuickTranslateVN_SingleInstance_Mutex", out bool createdNew);
        if (!createdNew)
        {
            System.Windows.MessageBox.Show(
                "QuickTranslateVN đang chạy sẵn dưới khay hệ thống (System Tray)!\n\n" +
                "• Bôi đen chữ -> Bấm nút [文A] cạnh chuột\n" +
                "• Bôi đen chữ -> Nhấn Alt + Q hoặc nhấn đúp Ctrl + C + C\n" +
                "• Nhấn Ctrl + Shift + T để mở khung tự nhập chữ dịch nhanh\n" +
                "• Nhấn Alt + S để quét dịch chữ trên ảnh / PDF scan",
                "QuickTranslateVN Đang Hoạt Động",
                MessageBoxButton.OK,
                MessageBoxImage.Information);
            Shutdown();
            return;
        }

        _settings = AppSettings.Load();
        _historyService = new HistoryService();
        _captureService = new TextCaptureService();
        _translationService = new TranslationService();
        _ocrService = new ScreenOcrService();

        _badgeWindow = new FloatingBadgeWindow();
        _popupWindow = new TranslationPopupWindow(
            _translationService,
            _captureService,
            _historyService,
            _settings,
            OpenSettingsDialog);

        _popupWindow.PopupHidden += () => MemoryOptimizer.ScheduleIdleTrim(_settings.AutoTrimMemory);

        _inputManager = new GlobalInputManager
        {
            EnableFloatingBadge = _settings.ShowFloatingBadgeOnSelect,
            EnableDoubleCtrlC = _settings.EnableDoubleCtrlC
        };

        WireEvents();
        _inputManager.Initialize();
        InitializeSystemTray();

        // Initial gentle memory trim after startup finishes
        MemoryOptimizer.ScheduleIdleTrim(_settings.AutoTrimMemory, delayMs: 2500);

        _trayIcon?.ShowBalloonTip(
            4000,
            "QuickTranslateVN v1.2 sẵn sàng!",
            "• Alt+Q / Ctrl+C+C / Nút [文A]: Dịch vùng chọn\n• Ctrl+Shift+T: Nhập chữ nhanh | Alt+S: Quét ảnh OCR",
            ToolTipIcon.Info);
    }

    private void WireEvents()
    {
        _inputManager.TextSelectionSuspected += pt =>
        {
            if (_isBusyCapturing || _isOcrOpen) return;
            _badgeWindow.ShowAt(pt);
        };

        _badgeWindow.BadgeClicked += async pt =>
        {
            await CaptureAndShowPopupAsync(pt);
        };

        _inputManager.QuickTranslateHotkeyPressed += async () =>
        {
            _badgeWindow.HideBadge();
            await CaptureAndShowPopupAsync(null);
        };

        // Ctrl + Shift + T: If text is selected, translate it; otherwise open Quick Input box!
        _inputManager.QuickInputHotkeyPressed += async () =>
        {
            if (_isBusyCapturing || _isOcrOpen) return;
            _badgeWindow.HideBadge();
            NativeMethods.GetCursorPos(out var pt);

            var captured = await _captureService.CaptureSelectedTextAsync(pt);
            if (captured != null && !string.IsNullOrWhiteSpace(captured.Text))
            {
                await _popupWindow.TranslateAndShowAsync(
                    captured.Text,
                    captured.Method,
                    captured.AnchorPoint,
                    _settings.CleanPdfLineBreaks);
            }
            else
            {
                _popupWindow.OpenQuickInputMode(pt);
            }
        };

        _inputManager.DoubleCopyTriggered += async () =>
        {
            if (_isBusyCapturing || _isOcrOpen) return;
            _badgeWindow.HideBadge();
            _isBusyCapturing = true;
            try
            {
                var captured = await _captureService.ReadCurrentClipboardSelectionAsync();
                if (captured != null)
                {
                    await _popupWindow.TranslateAndShowAsync(
                        captured.Text,
                        captured.Method,
                        captured.AnchorPoint,
                        _settings.CleanPdfLineBreaks);
                }
            }
            finally
            {
                _isBusyCapturing = false;
            }
        };

        _inputManager.OcrHotkeyPressed += async () =>
        {
            await RunScreenOcrFlowAsync();
        };

        _inputManager.ExternalMouseAction += _ =>
        {
            _badgeWindow.HideBadge();
            _popupWindow.DismissIfUnpinned();
        };

        _inputManager.EscapePressed += () =>
        {
            _badgeWindow.HideBadge();
            _popupWindow.DismissIfUnpinned();
        };
    }

    private async Task CaptureAndShowPopupAsync(NativeMethods.POINT? preferredAnchor)
    {
        if (_isBusyCapturing || _isOcrOpen) return;
        _isBusyCapturing = true;
        try
        {
            var captured = await _captureService.CaptureSelectedTextAsync(preferredAnchor);
            if (captured != null && !string.IsNullOrWhiteSpace(captured.Text))
            {
                await _popupWindow.TranslateAndShowAsync(
                    captured.Text,
                    captured.Method,
                    captured.AnchorPoint,
                    _settings.CleanPdfLineBreaks);
            }
            else
            {
                NativeMethods.GetCursorPos(out var pt);
                _popupWindow.OpenQuickInputMode(preferredAnchor ?? pt);
            }
        }
        finally
        {
            _isBusyCapturing = false;
        }
    }

    private async Task RunScreenOcrFlowAsync()
    {
        if (_isOcrOpen) return;
        _isOcrOpen = true;
        _badgeWindow.HideBadge();
        _popupWindow.DismissIfUnpinned();

        try
        {
            await Task.Delay(60);

            var overlay = new OcrOverlayWindow();
            bool? ok = overlay.ShowDialog();
            if (ok == true && overlay.SelectedPhysicalRect is { } rect)
            {
                await Task.Delay(40);
                var ocrText = await _ocrService.CaptureAndRecognizeAsync(rect);
                if (!string.IsNullOrWhiteSpace(ocrText))
                {
                    await _popupWindow.TranslateAndShowAsync(
                        ocrText,
                        "Windows OCR",
                        overlay.EndCursorPoint,
                        _settings.CleanPdfLineBreaks);
                }
                else
                {
                    await _popupWindow.TranslateAndShowAsync(
                        "Không nhận diện được chữ trong vùng vừa chọn.",
                        "OCR Trống",
                        overlay.EndCursorPoint,
                        cleanPdfLineBreaks: false);
                }
            }
        }
        finally
        {
            _isOcrOpen = false;
            MemoryOptimizer.ScheduleIdleTrim(_settings.AutoTrimMemory);
        }
    }

    private void OpenSettingsDialog()
    {
        if (_settingsWindow != null)
        {
            _settingsWindow.Activate();
            return;
        }

        _settingsWindow = new SettingsWindow(_settings, _historyService, _translationService);
        _settingsWindow.Closed += (_, _) =>
        {
            _inputManager.EnableFloatingBadge = _settings.ShowFloatingBadgeOnSelect;
            _inputManager.EnableDoubleCtrlC = _settings.EnableDoubleCtrlC;
            _popupWindow.RefreshEngineBadge();
            SyncTrayCheckmarks();
            _settingsWindow = null;
            MemoryOptimizer.ScheduleIdleTrim(_settings.AutoTrimMemory);
        };
        _settingsWindow.Show();
        _settingsWindow.Activate();
    }

    private void SyncTrayCheckmarks()
    {
        if (_badgeMenuToggle != null) _badgeMenuToggle.Checked = _settings.ShowFloatingBadgeOnSelect;
        if (_dblCopyMenuToggle != null) _dblCopyMenuToggle.Checked = _settings.EnableDoubleCtrlC;
        if (_pdfCleanMenuToggle != null) _pdfCleanMenuToggle.Checked = _settings.CleanPdfLineBreaks;
        if (_startupMenuToggle != null) _startupMenuToggle.Checked = AppSettings.IsStartupEnabled();
    }

    private void InitializeSystemTray()
    {
        _generatedIcon = CreateModernTrayIcon();
        var menu = new ContextMenuStrip();

        var headerItem = new ToolStripMenuItem("🌐 QuickTranslateVN v1.2") { Enabled = false };
        menu.Items.Add(headerItem);
        menu.Items.Add(new ToolStripSeparator());

        var quickInputItem = new ToolStripMenuItem("✏️ Mở hộp nhập chữ dịch nhanh   [Ctrl + Shift + T]");
        quickInputItem.Click += (_, _) =>
        {
            NativeMethods.GetCursorPos(out var pt);
            _popupWindow.OpenQuickInputMode(pt);
        };
        menu.Items.Add(quickInputItem);

        var ocrItem = new ToolStripMenuItem("🔍 Khoanh vùng dịch màn hình (OCR)   [Alt + S]");
        ocrItem.Click += async (_, _) => await RunScreenOcrFlowAsync();
        menu.Items.Add(ocrItem);

        var settingsItem = new ToolStripMenuItem("⚙️ Cài đặt, Engine AI & Lịch sử dịch...");
        settingsItem.Click += (_, _) => OpenSettingsDialog();
        menu.Items.Add(settingsItem);

        menu.Items.Add(new ToolStripSeparator());

        _badgeMenuToggle = new ToolStripMenuItem("Hiện nút nổi [文A] khi kéo chuột bôi đen")
        {
            Checked = _settings.ShowFloatingBadgeOnSelect,
            CheckOnClick = true
        };
        _badgeMenuToggle.Click += (_, _) =>
        {
            _settings.ShowFloatingBadgeOnSelect = _badgeMenuToggle.Checked;
            _inputManager.EnableFloatingBadge = _badgeMenuToggle.Checked;
            _settings.Save();
        };
        menu.Items.Add(_badgeMenuToggle);

        _dblCopyMenuToggle = new ToolStripMenuItem("Dịch nhanh khi nhấn đúp Ctrl + C + C")
        {
            Checked = _settings.EnableDoubleCtrlC,
            CheckOnClick = true
        };
        _dblCopyMenuToggle.Click += (_, _) =>
        {
            _settings.EnableDoubleCtrlC = _dblCopyMenuToggle.Checked;
            _inputManager.EnableDoubleCtrlC = _dblCopyMenuToggle.Checked;
            _settings.Save();
        };
        menu.Items.Add(_dblCopyMenuToggle);

        _pdfCleanMenuToggle = new ToolStripMenuItem("Tự động nối dòng bị ngắt khi đọc PDF")
        {
            Checked = _settings.CleanPdfLineBreaks,
            CheckOnClick = true
        };
        _pdfCleanMenuToggle.Click += (_, _) =>
        {
            _settings.CleanPdfLineBreaks = _pdfCleanMenuToggle.Checked;
            _settings.Save();
        };
        menu.Items.Add(_pdfCleanMenuToggle);

        _startupMenuToggle = new ToolStripMenuItem("Khởi động cùng Windows")
        {
            Checked = AppSettings.IsStartupEnabled(),
            CheckOnClick = true
        };
        _startupMenuToggle.Click += (_, _) =>
        {
            AppSettings.SetStartupEnabled(_startupMenuToggle.Checked);
        };
        menu.Items.Add(_startupMenuToggle);

        menu.Items.Add(new ToolStripSeparator());

        var exitItem = new ToolStripMenuItem("Thoát QuickTranslateVN");
        exitItem.Click += (_, _) => Shutdown();
        menu.Items.Add(exitItem);

        _trayIcon = new NotifyIcon
        {
            Icon = _generatedIcon,
            Text = "QuickTranslateVN (Alt+Q: Dịch chọn | Ctrl+Shift+T: Nhập | Alt+S: OCR)",
            Visible = true,
            ContextMenuStrip = menu
        };

        _trayIcon.DoubleClick += (_, _) =>
        {
            NativeMethods.GetCursorPos(out var pt);
            _popupWindow.OpenQuickInputMode(pt);
        };
    }

    private static Icon CreateModernTrayIcon()
    {
        using var bmp = new Bitmap(32, 32);
        using (var g = Graphics.FromImage(bmp))
        {
            g.SmoothingMode = SmoothingMode.AntiAlias;
            g.Clear(Color.Transparent);

            using var brush = new LinearGradientBrush(
                new Rectangle(2, 2, 28, 28),
                Color.FromArgb(37, 99, 235),
                Color.FromArgb(79, 70, 229),
                45f);

            using var path = new GraphicsPath();
            int r = 10;
            path.AddArc(2, 2, r, r, 180, 90);
            path.AddArc(30 - r, 2, r, r, 270, 90);
            path.AddArc(30 - r, 30 - r, r, r, 0, 90);
            path.AddArc(2, 30 - r, r, r, 90, 90);
            path.CloseFigure();
            g.FillPath(brush, path);

            using var font = new Font("Segoe UI", 11.5f, System.Drawing.FontStyle.Bold, GraphicsUnit.Pixel);
            using var whiteBrush = new SolidBrush(Color.White);
            var sf = new StringFormat
            {
                Alignment = StringAlignment.Center,
                LineAlignment = StringAlignment.Center
            };
            g.DrawString("VN", font, whiteBrush, new RectangleF(1, 2, 30, 28), sf);
        }

        IntPtr hIcon = bmp.GetHicon();
        return Icon.FromHandle(hIcon);
    }

    protected override void OnExit(ExitEventArgs e)
    {
        if (_trayIcon != null)
        {
            _trayIcon.Visible = false;
            _trayIcon.Dispose();
        }
        _inputManager?.Dispose();
        _captureService?.Dispose();
        _translationService?.Dispose();
        _singleInstanceMutex?.Dispose();
        base.OnExit(e);
    }
}
