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
    private GlobalInputManager _inputManager = null!;
    private TextCaptureService _captureService = null!;
    private TranslationService _translationService = null!;

    private FloatingBadgeWindow _badgeWindow = null!;
    private TranslationPopupWindow _popupWindow = null!;
    private bool _isBusyCapturing;

    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);

        _singleInstanceMutex = new Mutex(true, "Local\\QuickTranslateVN_SingleInstance_Mutex", out bool createdNew);
        if (!createdNew)
        {
            Shutdown();
            return;
        }

        _settings = AppSettings.Load();
        _captureService = new TextCaptureService();
        _translationService = new TranslationService();

        _badgeWindow = new FloatingBadgeWindow();
        _popupWindow = new TranslationPopupWindow(_translationService, _captureService);
        _popupWindow.PopupHidden += () => MemoryOptimizer.ScheduleIdleTrim(enabled: true);

        _inputManager = new GlobalInputManager
        {
            EnableFloatingBadge = _settings.ShowFloatingBadgeOnSelect,
            EnableDoubleCtrlC = _settings.EnableDoubleCtrlC
        };

        WireEvents();
        _inputManager.Initialize();
        InitializeSystemTray();

        MemoryOptimizer.ScheduleIdleTrim(enabled: true, delayMs: 1500);
    }

    private void WireEvents()
    {
        _inputManager.TextSelectionSuspected += pt =>
        {
            if (!_isBusyCapturing)
                _badgeWindow.ShowAt(pt);
        };

        _badgeWindow.BadgeClicked += async pt => await CaptureAndShowPopupAsync(pt);

        _inputManager.QuickTranslateHotkeyPressed += async () =>
        {
            _badgeWindow.HideBadge();
            await CaptureAndShowPopupAsync(null);
        };

        _inputManager.DoubleCopyTriggered += async () =>
        {
            if (_isBusyCapturing) return;
            _badgeWindow.HideBadge();
            _isBusyCapturing = true;
            try
            {
                var captured = await _captureService.ReadCurrentClipboardSelectionAsync();
                if (captured != null)
                {
                    await _popupWindow.TranslateAndShowAsync(
                        captured.Text,
                        captured.AnchorPoint,
                        _settings.CleanPdfLineBreaks);
                }
            }
            finally
            {
                _isBusyCapturing = false;
            }
        };

        _inputManager.ExternalMouseAction += () =>
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
        if (_isBusyCapturing) return;
        _isBusyCapturing = true;
        try
        {
            var captured = await _captureService.CaptureSelectedTextAsync(preferredAnchor);
            if (captured != null && !string.IsNullOrWhiteSpace(captured.Text))
            {
                await _popupWindow.TranslateAndShowAsync(
                    captured.Text,
                    captured.AnchorPoint,
                    _settings.CleanPdfLineBreaks);
            }
        }
        finally
        {
            _isBusyCapturing = false;
        }
    }

    private void InitializeSystemTray()
    {
        _generatedIcon = CreateTrayIcon();
        var menu = new ContextMenuStrip
        {
            Renderer = new ToolStripProfessionalRenderer(new CleanWhiteMenuColors()),
            BackColor = Color.White,
            ForeColor = Color.FromArgb(15, 23, 42),
            Font = new Font("Segoe UI", 9.25f),
            ShowImageMargin = true,
            Padding = new Padding(4)
        };

        var badgeToggle = new ToolStripMenuItem("Hiện nút [文A] khi bôi đen chuột")
        {
            Checked = _settings.ShowFloatingBadgeOnSelect,
            CheckOnClick = true,
            Padding = new Padding(2, 4, 6, 4)
        };
        badgeToggle.Click += (_, _) =>
        {
            _settings.ShowFloatingBadgeOnSelect = badgeToggle.Checked;
            _inputManager.EnableFloatingBadge = badgeToggle.Checked;
            _settings.Save();
        };
        menu.Items.Add(badgeToggle);

        var dblCopyToggle = new ToolStripMenuItem("Dịch khi nhấn đúp Ctrl + C + C")
        {
            Checked = _settings.EnableDoubleCtrlC,
            CheckOnClick = true,
            Padding = new Padding(2, 4, 6, 4)
        };
        dblCopyToggle.Click += (_, _) =>
        {
            _settings.EnableDoubleCtrlC = dblCopyToggle.Checked;
            _inputManager.EnableDoubleCtrlC = dblCopyToggle.Checked;
            _settings.Save();
        };
        menu.Items.Add(dblCopyToggle);

        var startupToggle = new ToolStripMenuItem("Khởi động cùng Windows")
        {
            Checked = AppSettings.IsStartupEnabled(),
            CheckOnClick = true,
            Padding = new Padding(2, 4, 6, 4)
        };
        startupToggle.Click += (_, _) => AppSettings.SetStartupEnabled(startupToggle.Checked);
        menu.Items.Add(startupToggle);

        menu.Items.Add(new ToolStripSeparator());

        var exitItem = new ToolStripMenuItem("Thoát")
        {
            Padding = new Padding(2, 4, 6, 4)
        };
        exitItem.Click += (_, _) => Shutdown();
        menu.Items.Add(exitItem);

        _trayIcon = new NotifyIcon
        {
            Icon = _generatedIcon,
            Text = "QuickTranslateVN (Bôi đen chữ hoặc nhấn Alt+Q / Ctrl+C+C)",
            Visible = true,
            ContextMenuStrip = menu
        };
    }

    private sealed class CleanWhiteMenuColors : ProfessionalColorTable
    {
        public CleanWhiteMenuColors() => UseSystemColors = false;
        public override Color ToolStripDropDownBackground => Color.White;
        public override Color ImageMarginGradientBegin => Color.White;
        public override Color ImageMarginGradientMiddle => Color.White;
        public override Color ImageMarginGradientEnd => Color.White;
        public override Color MenuBorder => Color.FromArgb(226, 232, 240);
        public override Color MenuItemBorder => Color.FromArgb(191, 219, 254);
        public override Color MenuItemSelected => Color.FromArgb(239, 246, 255);
        public override Color MenuItemSelectedGradientBegin => Color.FromArgb(239, 246, 255);
        public override Color MenuItemSelectedGradientEnd => Color.FromArgb(239, 246, 255);
        public override Color CheckBackground => Color.FromArgb(239, 246, 255);
        public override Color CheckSelectedBackground => Color.FromArgb(219, 234, 254);
        public override Color SeparatorDark => Color.FromArgb(241, 245, 249);
        public override Color SeparatorLight => Color.White;
    }

    private static Icon CreateTrayIcon()
    {
        using var bmp = new Bitmap(32, 32);
        using (var g = Graphics.FromImage(bmp))
        {
            g.SmoothingMode = SmoothingMode.AntiAlias;
            g.Clear(Color.Transparent);

            using var bgBrush = new SolidBrush(Color.White);
            using var borderPen = new Pen(Color.FromArgb(37, 99, 235), 2f);

            using var path = new GraphicsPath();
            const int r = 10;
            path.AddArc(2, 2, r, r, 180, 90);
            path.AddArc(30 - r, 2, r, r, 270, 90);
            path.AddArc(30 - r, 30 - r, r, r, 0, 90);
            path.AddArc(2, 30 - r, r, r, 90, 90);
            path.CloseFigure();
            g.FillPath(bgBrush, path);
            g.DrawPath(borderPen, path);

            using var font = new Font("Segoe UI", 11.5f, System.Drawing.FontStyle.Bold, GraphicsUnit.Pixel);
            using var blueBrush = new SolidBrush(Color.FromArgb(37, 99, 235));
            var sf = new StringFormat { Alignment = StringAlignment.Center, LineAlignment = StringAlignment.Center };
            g.DrawString("VN", font, blueBrush, new RectangleF(1, 2, 30, 28), sf);
        }

        return Icon.FromHandle(bmp.GetHicon());
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
