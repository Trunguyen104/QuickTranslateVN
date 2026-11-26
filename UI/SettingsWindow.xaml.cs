using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using QuickTranslateVN.Services;

namespace QuickTranslateVN.UI;

public partial class SettingsWindow : Window
{
    private readonly AppSettings _settings;
    private readonly HistoryService _historyService;
    private readonly TranslationService _translationService;

    public SettingsWindow(AppSettings settings, HistoryService historyService, TranslationService translationService)
    {
        _settings = settings;
        _historyService = historyService;
        _translationService = translationService;

        InitializeComponent();
        LoadFromSettings();
        RefreshHistoryList();
    }

    private void LoadFromSettings()
    {
        ChkFloatingBadge.IsChecked = _settings.ShowFloatingBadgeOnSelect;
        ChkDoubleCtrlC.IsChecked = _settings.EnableDoubleCtrlC;
        ChkCleanPdf.IsChecked = _settings.CleanPdfLineBreaks;
        ChkBackTranslate.IsChecked = _settings.EnableBackTranslation;
        ChkSaveHistory.IsChecked = _settings.SaveHistory;
        ChkAutoTrimMemory.IsChecked = _settings.AutoTrimMemory;
        ChkStartup.IsChecked = AppSettings.IsStartupEnabled();

        string providerTag = _settings.Provider.ToString();
        foreach (ComboBoxItem item in CmbProvider.Items)
        {
            if (string.Equals(item.Tag?.ToString(), providerTag, StringComparison.OrdinalIgnoreCase))
            {
                CmbProvider.SelectedItem = item;
                break;
            }
        }
        if (CmbProvider.SelectedIndex < 0)
            CmbProvider.SelectedIndex = 0;

        PwdDeepLKey.Password = _settings.DeepLApiKey;
        TxtLlmEndpoint.Text = _settings.LlmEndpoint;
        TxtLlmModel.Text = _settings.LlmModel;
        PwdLlmKey.Password = _settings.LlmApiKey;
    }

    private void RefreshHistoryList()
    {
        LstHistory.ItemsSource = _historyService.GetItems();
    }

    private void BtnPresetOllama_Click(object sender, RoutedEventArgs e)
    {
        TxtLlmEndpoint.Text = "http://localhost:11434/v1/chat/completions";
        TxtLlmModel.Text = "qwen2.5:7b";
        PwdLlmKey.Password = string.Empty;
        CmbProvider.SelectedIndex = 3;
    }

    private void BtnPresetDeepSeek_Click(object sender, RoutedEventArgs e)
    {
        TxtLlmEndpoint.Text = "https://api.deepseek.com/chat/completions";
        TxtLlmModel.Text = "deepseek-chat";
        CmbProvider.SelectedIndex = 3;
    }

    private void BtnClearHistory_Click(object sender, RoutedEventArgs e)
    {
        _historyService.Clear();
        _translationService.ClearCache();
        RefreshHistoryList();
    }

    private void LstHistory_MouseDoubleClick(object sender, MouseButtonEventArgs e)
    {
        if (LstHistory.SelectedItem is HistoryItem item && !string.IsNullOrEmpty(item.TranslatedText))
        {
            try
            {
                System.Windows.Clipboard.SetText(item.TranslatedText);
                Title = "✓ Đã sao chép bản dịch vào Clipboard!";
            }
            catch
            {
                // Ignore clipboard lock
            }
        }
    }

    private void BtnSave_Click(object sender, RoutedEventArgs e)
    {
        _settings.ShowFloatingBadgeOnSelect = ChkFloatingBadge.IsChecked == true;
        _settings.EnableDoubleCtrlC = ChkDoubleCtrlC.IsChecked == true;
        _settings.CleanPdfLineBreaks = ChkCleanPdf.IsChecked == true;
        _settings.EnableBackTranslation = ChkBackTranslate.IsChecked == true;
        _settings.SaveHistory = ChkSaveHistory.IsChecked == true;
        _settings.AutoTrimMemory = ChkAutoTrimMemory.IsChecked == true;

        if (CmbProvider.SelectedItem is ComboBoxItem selected &&
            Enum.TryParse<TranslationProviderType>(selected.Tag?.ToString(), out var parsedProvider))
        {
            _settings.Provider = parsedProvider;
        }

        _settings.DeepLApiKey = PwdDeepLKey.Password;
        _settings.LlmEndpoint = TxtLlmEndpoint.Text.Trim();
        _settings.LlmModel = TxtLlmModel.Text.Trim();
        _settings.LlmApiKey = PwdLlmKey.Password;

        _settings.Save();
        AppSettings.SetStartupEnabled(ChkStartup.IsChecked == true);

        DialogResult = true;
        Close();
    }

    private void BtnCancel_Click(object sender, RoutedEventArgs e)
    {
        DialogResult = false;
        Close();
    }
}
