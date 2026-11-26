using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using QuickTranslateVN.Interop;
using QuickTranslateVN.Services;

namespace QuickTranslateVN.UI;

public partial class TranslationPopupWindow : Window
{
    private static readonly SolidColorBrush BrushTextReady = FreezeBrush("#0F172A");
    private static readonly SolidColorBrush BrushTextLoading = FreezeBrush("#64748B");
    private static readonly SolidColorBrush BrushTextError = FreezeBrush("#DC2626");
    private static readonly SolidColorBrush BrushPosBg = FreezeBrush("#EFF6FF");
    private static readonly SolidColorBrush BrushPosBorder = FreezeBrush("#BFDBFE");
    private static readonly SolidColorBrush BrushPosFg = FreezeBrush("#2563EB");
    private static readonly SolidColorBrush BrushMeanings = FreezeBrush("#334155");
    private static readonly SolidColorBrush BrushPinnedBg = FreezeBrush("#DBEAFE");
    private static readonly SolidColorBrush BrushPinnedFg = FreezeBrush("#2563EB");
    private static readonly SolidColorBrush BrushIconDefaultFg = FreezeBrush("#64748B");

    private readonly TranslationService _translationService;
    private readonly TextCaptureService _captureService;
    private CancellationTokenSource? _cts;

    private string _currentOriginal = string.Empty;
    private string _currentTranslated = string.Empty;
    private string _currentSourceLang = "en";
    private string _currentTargetLang = "vi";

    private bool _isDragging;
    private NativeMethods.POINT _dragStartCursor;
    private double _dragStartLeft;
    private double _dragStartTop;

    public bool IsPinned { get; private set; }
    public event Action? PopupHidden;

    public TranslationPopupWindow(TranslationService translationService, TextCaptureService captureService)
    {
        _translationService = translationService;
        _captureService = captureService;
        InitializeComponent();
        WindowPositionHelper.MakeNonActivating(this);
    }

    private static SolidColorBrush FreezeBrush(string hex)
    {
        var b = new SolidColorBrush((System.Windows.Media.Color)System.Windows.Media.ColorConverter.ConvertFromString(hex));
        b.Freeze();
        return b;
    }

    public async Task TranslateAndShowAsync(
        string rawText,
        NativeMethods.POINT anchorPt,
        bool cleanPdfLineBreaks,
        string? forceTargetLang = null,
        bool reposition = true)
    {
        _cts?.Cancel();
        _cts = new CancellationTokenSource();
        var token = _cts.Token;

        string cleaned = TextCleaner.Clean(rawText, cleanPdfLineBreaks);
        if (string.IsNullOrWhiteSpace(cleaned))
            return;

        _currentOriginal = cleaned;
        _currentTranslated = string.Empty;

        TxtTranslated.Foreground = BrushTextLoading;
        TxtTranslated.Text = "Đang dịch...";
        TxtPhonetic.Visibility = Visibility.Collapsed;
        DictDivider.Visibility = Visibility.Collapsed;
        PanelDictionary.Visibility = Visibility.Collapsed;
        PanelDictionary.Children.Clear();
        TxtSpeed.Text = string.Empty;
        IconCopyNormal.Visibility = Visibility.Visible;
        IconCopyDone.Visibility = Visibility.Collapsed;

        if (reposition)
        {
            UpdateLayout();
            WindowPositionHelper.ShowAndPositionNearPhysicalPoint(this, anchorPt, offsetDipX: 8, offsetDipY: 14);
        }

        try
        {
            var result = await _translationService.TranslateAsync(cleaned, forceTargetLang, token);
            if (token.IsCancellationRequested) return;

            _currentTranslated = result.TranslatedText;
            _currentSourceLang = result.SourceLang;
            _currentTargetLang = result.TargetLang;

            TxtLangBadge.Text = $"{result.SourceLang.ToUpperInvariant()} → {result.TargetLang.ToUpperInvariant()}  ⇄";
            TxtSpeed.Text = result.ElapsedMs > 0 ? $"{result.ElapsedMs}ms" : "cache";

            TxtTranslated.Foreground = BrushTextReady;
            TxtTranslated.Text = result.TranslatedText;

            if (!string.IsNullOrWhiteSpace(result.Phonetic))
            {
                TxtPhonetic.Text = $"/{result.Phonetic}/";
                TxtPhonetic.Visibility = Visibility.Visible;
            }

            if (result.DictionaryEntries.Count > 0)
            {
                RenderDictionaryEntries(result.DictionaryEntries);
                DictDivider.Visibility = Visibility.Visible;
                PanelDictionary.Visibility = Visibility.Visible;
            }

            UpdateLayout();
            if (reposition && !_isDragging && !IsPinned)
            {
                WindowPositionHelper.ShowAndPositionNearPhysicalPoint(this, anchorPt, offsetDipX: 8, offsetDipY: 14);
            }
        }
        catch (OperationCanceledException) { }
        catch
        {
            TxtTranslated.Foreground = BrushTextError;
            TxtTranslated.Text = "Không thể kết nối mạng để dịch.";
        }
    }

    private void RenderDictionaryEntries(List<DictionaryEntryItem> entries)
    {
        PanelDictionary.Children.Clear();
        foreach (var entry in entries.Take(3))
        {
            var row = new WrapPanel { Margin = new Thickness(0, 2, 0, 2) };
            if (!string.IsNullOrWhiteSpace(entry.PartOfSpeech))
            {
                row.Children.Add(new Border
                {
                    Background = BrushPosBg,
                    BorderBrush = BrushPosBorder,
                    BorderThickness = new Thickness(1),
                    CornerRadius = new CornerRadius(4),
                    Padding = new Thickness(5, 1, 5, 1),
                    Margin = new Thickness(0, 0, 6, 2),
                    Child = new TextBlock
                    {
                        Text = entry.PartOfSpeech,
                        Foreground = BrushPosFg,
                        FontFamily = new System.Windows.Media.FontFamily("Segoe UI Semibold"),
                        FontSize = 10.5
                    }
                });
            }

            row.Children.Add(new TextBlock
            {
                Text = string.Join(", ", entry.Meanings),
                Foreground = BrushMeanings,
                FontSize = 12,
                TextWrapping = TextWrapping.Wrap,
                VerticalAlignment = VerticalAlignment.Center
            });
            PanelDictionary.Children.Add(row);
        }
    }

    public void DismissIfUnpinned()
    {
        if (!IsPinned && IsVisible)
            ForceDismiss();
    }

    public void ForceDismiss()
    {
        _cts?.Cancel();
        _translationService.CleanupTempAudio();
        if (IsVisible)
        {
            Hide();
            PopupHidden?.Invoke();
        }
    }

    private async void BtnSwapLang_Click(object sender, RoutedEventArgs e)
    {
        if (string.IsNullOrWhiteSpace(_currentOriginal)) return;
        string nextTarget = _currentTargetLang == "vi" ? "en" : "vi";
        await TranslateAndShowAsync(_currentOriginal, default, cleanPdfLineBreaks: false, forceTargetLang: nextTarget, reposition: false);
    }

    private async void BtnSpeak_Click(object sender, RoutedEventArgs e)
    {
        await _translationService.SpeakAsync(_currentOriginal, _currentSourceLang);
    }

    private async void BtnReplace_Click(object sender, RoutedEventArgs e)
    {
        if (string.IsNullOrEmpty(_currentTranslated)) return;
        string toInsert = _currentTranslated;
        ForceDismiss();
        await Task.Delay(30);
        await _captureService.ReplaceSelectedTextInActiveAppAsync(toInsert);
    }

    private async void BtnCopy_Click(object sender, RoutedEventArgs e)
    {
        if (string.IsNullOrEmpty(_currentTranslated)) return;
        try
        {
            System.Windows.Clipboard.SetText(_currentTranslated);
            IconCopyNormal.Visibility = Visibility.Collapsed;
            IconCopyDone.Visibility = Visibility.Visible;
            await Task.Delay(1000);
            IconCopyDone.Visibility = Visibility.Collapsed;
            IconCopyNormal.Visibility = Visibility.Visible;
        }
        catch { }
    }

    private void BtnPin_Click(object sender, RoutedEventArgs e)
    {
        IsPinned = !IsPinned;
        BtnPin.Background = IsPinned ? BrushPinnedBg : System.Windows.Media.Brushes.Transparent;
        BtnPin.Foreground = IsPinned ? BrushPinnedFg : BrushIconDefaultFg;
    }

    private void BtnClose_Click(object sender, RoutedEventArgs e) => ForceDismiss();

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
        Left = _dragStartLeft + ((current.X - _dragStartCursor.X) / dpi.DpiScaleX);
        Top = _dragStartTop + ((current.Y - _dragStartCursor.Y) / dpi.DpiScaleY);
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
