using System.IO;
using System.Text.Json;
using Microsoft.Win32;

namespace QuickTranslateVN.Services;

public sealed class AppSettings
{
    public bool ShowFloatingBadgeOnSelect { get; set; } = true;
    public bool EnableDoubleCtrlC { get; set; } = true;
    public bool CleanPdfLineBreaks { get; set; } = true;

    private static readonly string SettingsPath = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
        "QuickTranslateVN",
        "settings.json");

    private const string StartupRegistryKey = @"Software\Microsoft\Windows\CurrentVersion\Run";
    private const string AppRegistryName = "QuickTranslateVN";

    public static AppSettings Load()
    {
        try
        {
            if (File.Exists(SettingsPath))
                return JsonSerializer.Deserialize<AppSettings>(File.ReadAllText(SettingsPath)) ?? new AppSettings();
        }
        catch { }
        return new AppSettings();
    }

    public void Save()
    {
        try
        {
            Directory.CreateDirectory(Path.GetDirectoryName(SettingsPath)!);
            File.WriteAllText(SettingsPath, JsonSerializer.Serialize(this));
        }
        catch { }
    }

    public static bool IsStartupEnabled()
    {
        try
        {
            using var key = Registry.CurrentUser.OpenSubKey(StartupRegistryKey, false);
            return key?.GetValue(AppRegistryName) != null;
        }
        catch { return false; }
    }

    public static void SetStartupEnabled(bool enable)
    {
        try
        {
            using var key = Registry.CurrentUser.OpenSubKey(StartupRegistryKey, true);
            if (key == null) return;
            if (enable && !string.IsNullOrEmpty(Environment.ProcessPath))
                key.SetValue(AppRegistryName, $"\"{Environment.ProcessPath}\"");
            else
                key.DeleteValue(AppRegistryName, false);
        }
        catch { }
    }
}
