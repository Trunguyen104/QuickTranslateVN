using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using QuickTranslateVN.Interop;
using QuickTranslateVN.Services;

namespace QuickTranslateVN.UI;

public partial class TranslationPopupWindow : Window
{
    private readonly TranslationService _translationService;
    private readonly TextCaptureService _captureService;
    private readonly HistoryService _historyService;
    private readonly AppSettings _settings;
    private readonly Action _openSettingsAction;

    private CancellationTokenSource? _cts;

    private string _currentOriginal = string.Empty;
    private string _currentTranslated = string.Empty;
    private string _currentSourceLang = "en";
    private string _currentTargetLang = "vi";
    private string _currentCaptureMethod = "";
    private bool _isInputMode;

    private bool _isDragging;
    private NativeMethods.POINT _dragStartCursor;
    private double _dragStartLeft;
    private double _dragStartTop;

    public bool IsPinned { get; private set; }
    public event Action? PopupHidden;

    public TranslationPopupWindow(
        TranslationService translationService,
        TextCaptureService captureService,
        HistoryService historyService,
        AppSettings settings,
        Action openSettingsAction)
    {
        _translationService = translationService;
        _captureService = captureService;
        _historyService = historyService;
        _settings = settings;
        _openSettingsAction = openSettingsAction;

        InitializeComponent();
        WindowPositionHelper.MakeNonActivating(this);
        RefreshEngineBadge();
    }

    public void RefreshEngineBadge()
    {
        TxtEngineBadge.Text = _settings.Provider switch
        {
            TranslationProviderType.MicrosoftFree => "Microsoft",
            TranslationProviderType.DeepL => "DeepL",
            TranslationProviderType.OpenAiCompatible => "AI / LLM",
            _ => "Google"
        };
    }

    public void OpenQuickInputMode(NativeMethods.POINT anchorPt, string? initialText = null)
    {
        _isInputMode = true;
        InputEditorBox.Visibility = Visibility.Visible;
        SourcePreviewContainer.Visibility = Visibility.Collapsed;
        BtnReplace.Visibility = Visibility.Collapsed;

        RefreshEngineBadge();
        TxtSourceEditor.Text = initialText ?? string.Empty;

        if (string.IsNullOrWhiteSpace(initialText))
        {
            TxtTranslated.Foreground = new SolidColorBrush((System.Windows.Media.Color)System.Windows.Media.ColorConverter.ConvertFromString("#94A3B8"));
            TxtTranslated.Text = "Nhập câu tiếng Anh hoặc tiếng Việt ở trên rồi nhấn Ctrl + Enter...";
            TxtPhonetic.Visibility = Visibility.Collapsed;
            PanelDictionary.Visibility = Visibility.Collapsed;
            BackTransContainer.Visibility = Visibility.Collapsed;
            TxtMetaInfo.Text = "• Nhập nhanh";
        }

        UpdateLayout();
        WindowPositionHelper.ShowAndPositionNearPhysicalPoint(this, anchorPt, offsetDipX: 8, offsetDipY: 16);
        WindowPositionHelper.SetInteractiveFocusMode(this, interactive: true);
        TxtSourceEditor.Focus();
        TxtSourceEditor.CaretIndex = TxtSourceEditor.Text.Length;

        if (!string.IsNullOrWhiteSpace(initialText))
        {
            _ = TranslateInternalAsync(initialText, "Nhập nhanh", anchorPt, cleanPdfLineBreaks: false, repositionAfter: false);
        }
    }

    public async Task TranslateAndShowAsync(
        string rawText,
        string captureMethod,
        NativeMethods.POINT anchorPt,
        bool cleanPdfLineBreaks,
        string? forceTargetLang = null)
    {
        _isInputMode = false;
        WindowPositionHelper.SetInteractiveFocusMode(this, interactive: false);
        InputEditorBox.Visibility = Visibility.Collapsed;
        SourcePreviewContainer.Visibility = Visibility.Visible;
        BtnReplace.Visibility = Visibility.Visible;
        BtnReplace.Content = "⏎ Thay thế";

        await TranslateInternalAsync(rawText, captureMethod, anchorPt, cleanPdfLineBreaks, forceTargetLang, repositionAfter: true);
    }

    private async Task TranslateInternalAsync(
        string rawText,
        string captureMethod,
        NativeMethods.POINT anchorPt,
        bool cleanPdfLineBreaks,
        string? forceTargetLang = null,
        bool repositionAfter = true)
    {
        _cts?.Cancel();
        _cts = new CancellationTokenSource();
        var token = _cts.Token;

        string cleaned = TextCleaner.Clean(rawText, cleanPdfLineBreaks);
        if (string.IsNullOrWhiteSpace(cleaned))
            return;

        _currentOriginal = cleaned;
        _currentTranslated = string.Empty;
        _currentCaptureMethod = captureMethod;

        RefreshEngineBadge();
        TxtTranslated.Foreground = new SolidColorBrush((System.Windows.Media.Color)System.Windows.Media.ColorConverter.ConvertFromString("#94A3B8"));
        TxtTranslated.Text = "⏳ Đang dịch...";
        TxtPhonetic.Visibility = Visibility.Collapsed;
        PanelDictionary.Visibility = Visibility.Collapsed;
        PanelDictionary.Children.Clear();
        BackTransContainer.Visibility = Visibility.Collapsed;
        TxtOriginalPreview.Text = cleaned;
        TxtMetaInfo.Text = $"• {captureMethod}";
        BtnCopy.Content = "📋 Chép";

        if (repositionAfter)
        {
            UpdateLayout();
            WindowPositionHelper.ShowAndPositionNearPhysicalPoint(this, anchorPt, offsetDipX: 8, offsetDipY: 16);
        }
        else if (!IsVisible)
        {
            Show();
        }

        try
        {
            var result = await _translationService.TranslateAsync(cleaned, _settings, forceTargetLang, token);
            if (token.IsCancellationRequested) return;

            _currentTranslated = result.TranslatedText;
            _currentSourceLang = result.SourceLang;
            _currentTargetLang = result.TargetLang;

            TxtLangBadge.Text = $"{result.SourceLang.ToUpperInvariant()} → {result.TargetLang.ToUpperInvariant()} ⇄";
            TxtEngineBadge.Text = result.EngineName;
            TxtMetaInfo.Text = $"• {captureMethod} • {result.ElapsedMs}ms";

            TxtTranslated.Foreground = new SolidColorBrush((System.Windows.Media.Color)System.Windows.Media.ColorConverter.ConvertFromString("#F8FAFC"));
            TxtTranslated.Text = result.TranslatedText;

            if (!string.IsNullOrWhiteSpace(result.Phonetic))
            {
                TxtPhonetic.Text = $"/{result.Phonetic}/";
                TxtPhonetic.Visibility = Visibility.Visible;
            }

            if (!string.IsNullOrWhiteSpace(result.BackTranslatedText))
            {
                TxtBackTranslated.Text = $"↩ Dịch ngược: {result.BackTranslatedText}";
                BackTransContainer.Visibility = Visibility.Visible;
            }

            if (result.DictionaryEntries.Count > 0)
            {
                RenderDictionaryEntries(result.DictionaryEntries);
                PanelDictionary.Visibility = Visibility.Visible;
            }

            _historyService.Record(
                _currentOriginal,
                _currentTranslated,
                _currentSourceLang,
                _currentTargetLang,
                result.EngineName,
                _settings.SaveHistory);

            UpdateLayout();
            if (repositionAfter && !_isDragging && !IsPinned)
            {
                WindowPositionHelper.ShowAndPositionNearPhysicalPoint(this, anchorPt, offsetDipX: 8, offsetDipY: 16);
            }
        }
        catch (OperationCanceledException)
        {
            // Superseded
        }
        catch (Exception ex)
        {
            TxtTranslated.Foreground = new SolidColorBrush((System.Windows.Media.Color)System.Windows.Media.ColorConverter.ConvertFromString("#FCA5A5"));
            TxtTranslated.Text = $"Lỗi dịch ({TxtEngineBadge.Text}): {ex.Message}";
        }
    }

    private void RenderDictionaryEntries(List<DictionaryEntryItem> entries)
    {
        PanelDictionary.Children.Clear();

        foreach (var entry in entries.Take(4))
        {
            var row = new WrapPanel { Margin = new Thickness(0, 2.5, 0, 2.5) };

            if (!string.IsNullOrWhiteSpace(entry.PartOfSpeech))
            {
                var posBadge = new Border
                {
                    Background = new SolidColorBrush((System.Windows.Media.Color)System.Windows.Media.ColorConverter.ConvertFromString("#1E3A5F")),
                    CornerRadius = new CornerRadius(4),
                    Padding = new Thickness(6, 1, 6, 1),
                    Margin = new Thickness(0, 0, 6, 2),
                    Child = new TextBlock
                    {
                        Text = entry.PartOfSpeech,
                        Foreground = new SolidColorBrush((System.Windows.Media.Color)System.Windows.Media.ColorConverter.ConvertFromString("#93C5FD")),
                        FontSize = 11.5,
                        FontFamily = new System.Windows.Media.FontFamily("Segoe UI Semibold")
                    }
                };
                row.Children.Add(posBadge);
            }

            var termsBlock = new TextBlock
            {
                Text = string.Join(", ", entry.Meanings),
                Foreground = new SolidColorBrush((System.Windows.Media.Color)System.Windows.Media.ColorConverter.ConvertFromString("#CBD5E1")),
                FontSize = 12.5,
                TextWrapping = TextWrapping.Wrap,
                VerticalAlignment = VerticalAlignment.Center
            };
            row.Children.Add(termsBlock);

            PanelDictionary.Children.Add(row);
        }
    }

    public void DismissIfUnpinned()
    {
        if (!IsPinned && IsVisible)
        {
            ForceDismiss();
        }
    }

    public void ForceDismiss()
    {
        _cts?.Cancel();
        _translationService.CleanupTempAudio();
        if (_isInputMode)
        {
            _isInputMode = false;
            WindowPositionHelper.SetInteractiveFocusMode(this, interactive: false);
        }

        if (IsVisible)
        {
            Hide();
            PopupHidden?.Invoke();
        }
    }

    private NativeMethods.POINT CurrentWindowPhysicalPoint()
    {
        var dpi = VisualTreeHelper.GetDpi(this);
        return new NativeMethods.POINT
        {
            X = (int)(Left * dpi.DpiScaleX),
            Y = (int)(Top * dpi.DpiScaleY)
        };
    }

    private async void BtnSwapLang_Click(object sender, RoutedEventArgs e)
    {
        if (string.IsNullOrWhiteSpace(_currentOriginal)) return;
        string nextTarget = _currentTargetLang == "vi" ? "en" : "vi";
        await TranslateInternalAsync(_currentOriginal, _currentCaptureMethod, CurrentWindowPhysicalPoint(), cleanPdfLineBreaks: false, forceTargetLang: nextTarget, repositionAfter: false);
    }

    private async void BtnEngine_Click(object sender, RoutedEventArgs e)
    {
        // Quick cycle between Google Free -> Microsoft Free -> (DeepL if key exists) -> (AI if configured)
        _settings.Provider = _settings.Provider switch
        {
            TranslationProviderType.GoogleFree => TranslationProviderType.MicrosoftFree,
            TranslationProviderType.MicrosoftFree when !string.IsNullOrWhiteSpace(_settings.DeepLApiKey) => TranslationProviderType.DeepL,
            TranslationProviderType.MicrosoftFree => TranslationProviderType.OpenAiCompatible,
            TranslationProviderType.DeepL => TranslationProviderType.OpenAiCompatible,
            _ => TranslationProviderType.GoogleFree
        };
        _settings.Save();
        RefreshEngineBadge();

        if (!string.IsNullOrWhiteSpace(_currentOriginal))
        {
            await TranslateInternalAsync(_currentOriginal, _currentCaptureMethod, CurrentWindowPhysicalPoint(), cleanPdfLineBreaks: false, forceTargetLang: _currentTargetLang, repositionAfter: false);
        }
    }

    private void BtnEditInput_Click(object sender, RoutedEventArgs e)
    {
        if (_isInputMode)
        {
            _isInputMode = false;
            InputEditorBox.Visibility = Visibility.Collapsed;
            SourcePreviewContainer.Visibility = Visibility.Visible;
            WindowPositionHelper.SetInteractiveFocusMode(this, interactive: false);
        }
        else
        {
            _isInputMode = true;
            InputEditorBox.Visibility = Visibility.Visible;
            SourcePreviewContainer.Visibility = Visibility.Collapsed;
            TxtSourceEditor.Text = _currentOriginal;
            WindowPositionHelper.SetInteractiveFocusMode(this, interactive: true);
            TxtSourceEditor.Focus();
            TxtSourceEditor.SelectAll();
        }
    }

    private async void BtnTranslateInput_Click(object sender, RoutedEventArgs e)
    {
        var input = TxtSourceEditor.Text?.Trim();
        if (string.IsNullOrEmpty(input)) return;
        await TranslateInternalAsync(input, "Nhập tay", CurrentWindowPhysicalPoint(), cleanPdfLineBreaks: false, repositionAfter: false);
    }

    private async void TxtSourceEditor_KeyDown(object sender, System.Windows.Input.KeyEventArgs e)
    {
        if (e.Key == Key.Enter && (Keyboard.Modifiers & ModifierKeys.Control) == ModifierKeys.Control)
        {
            e.Handled = true;
            var input = TxtSourceEditor.Text?.Trim();
            if (!string.IsNullOrEmpty(input))
            {
                await TranslateInternalAsync(input, "Nhập tay", CurrentWindowPhysicalPoint(), cleanPdfLineBreaks: false, repositionAfter: false);
            }
        }
        else if (e.Key == Key.Escape)
        {
            e.Handled = true;
            ForceDismiss();
        }
    }

    private async void BtnBackTranslate_Click(object sender, RoutedEventArgs e)
    {
        if (string.IsNullOrWhiteSpace(_currentTranslated)) return;
        BtnBackTranslate.Content = "⏳...";
        try
        {
            using var cts = new CancellationTokenSource(TimeSpan.FromSeconds(6));
            var back = await _translationService.TryTranslateBackAsync(_currentTranslated, _currentSourceLang, cts.Token);
            if (!string.IsNullOrWhiteSpace(back))
            {
                TxtBackTranslated.Text = $"↩ Dịch ngược ({_currentSourceLang.ToUpperInvariant()}): {back}";
                BackTransContainer.Visibility = Visibility.Visible;
            }
        }
        finally
        {
            BtnBackTranslate.Content = "↩ Dịch ngược";
        }
    }

    private async void BtnReplace_Click(object sender, RoutedEventArgs e)
    {
        if (string.IsNullOrEmpty(_currentTranslated)) return;

        string toInsert = _currentTranslated;
        ForceDismiss();
        await Task.Delay(35);
        await _captureService.ReplaceSelectedTextInActiveAppAsync(toInsert);
    }

    private async void BtnSpeakSource_Click(object sender, RoutedEventArgs e)
    {
        await _translationService.SpeakAsync(_currentOriginal, _currentSourceLang);
    }

    private async void BtnSpeakTarget_Click(object sender, RoutedEventArgs e)
    {
        await _translationService.SpeakAsync(_currentTranslated, _currentTargetLang);
    }

    private async void BtnCopy_Click(object sender, RoutedEventArgs e)
    {
        if (string.IsNullOrEmpty(_currentTranslated)) return;
        try
        {
            System.Windows.Clipboard.SetText(_currentTranslated);
            BtnCopy.Content = "✓ Đã chép";
            await Task.Delay(1300);
            BtnCopy.Content = "📋 Chép";
        }
        catch
        {
            // Ignore clipboard contention
        }
    }

    private void BtnPin_Click(object sender, RoutedEventArgs e)
    {
        IsPinned = !IsPinned;
        BtnPin.Opacity = IsPinned ? 1.0 : 0.55;
        BtnPin.Background = IsPinned
            ? new SolidColorBrush((System.Windows.Media.Color)System.Windows.Media.ColorConverter.ConvertFromString("#1D4ED8"))
            : System.Windows.Media.Brushes.Transparent;
    }

    private void BtnSettings_Click(object sender, RoutedEventArgs e)
    {
        _openSettingsAction();
    }

    private void BtnClose_Click(object sender, RoutedEventArgs e)
    {
        ForceDismiss();
    }

    private void Header_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
    {
        if (e.OriginalSource is DependencyObject dep && FindParent<System.Windows.Controls.Button>(dep) != null)
            return;

        _isDragging = true;
        NativeMethods.GetCursorPos(out _dragStartCursor);
        _dragStartLeft = Left;
        _dragStartTop = Top;
        ((UIElement)sender).CaptureMouse();
        e.Handled = true;
    }

    private void Header_MouseMove(object sender, System.Windows.Input.MouseEventArgs e)
    {
        if (!_isDragging) return;
        NativeMethods.GetCursorPos(out var current);
        var dpi = VisualTreeHelper.GetDpi(this);
        double dx = (current.X - _dragStartCursor.X) / dpi.DpiScaleX;
        double dy = (current.Y - _dragStartCursor.Y) / dpi.DpiScaleY;
        Left = _dragStartLeft + dx;
        Top = _dragStartTop + dy;
    }

    private void Header_MouseLeftButtonUp(object sender, MouseButtonEventArgs e)
    {
        if (!_isDragging) return;
        _isDragging = false;
        ((UIElement)sender).ReleaseMouseCapture();
        e.Handled = true;
    }

    private static T? FindParent<T>(DependencyObject? child) where T : DependencyObject
    {
        while (child != null)
        {
            if (child is T typed) return typed;
            child = VisualTreeHelper.GetParent(child);
        }
        return null;
    }
}
