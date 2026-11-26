using System.IO;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;
using Microsoft.Win32;

namespace QuickTranslateVN.Services;

public enum TranslationProviderType
{
    GoogleFree,
    MicrosoftFree,
    DeepL,
    OpenAiCompatible
}

public sealed class AppSettings
{
    public bool ShowFloatingBadgeOnSelect { get; set; } = true;
    public bool EnableDoubleCtrlC { get; set; } = true;
    public bool CleanPdfLineBreaks { get; set; } = true;
    public bool EnableBackTranslation { get; set; } = false;
    public bool SaveHistory { get; set; } = true;
    public bool AutoTrimMemory { get; set; } = true;

    [JsonConverter(typeof(JsonStringEnumConverter))]
    public TranslationProviderType Provider { get; set; } = TranslationProviderType.GoogleFree;

    // Encrypted with Windows DPAPI (CurrentUser) so API keys are never stored in plain text on disk
    public string DeepLApiKeyEncrypted { get; set; } = string.Empty;

    // Supports 100% Offline Ollama (http://localhost:11434/v1/chat/completions) or OpenAI / DeepSeek / Groq
    public string LlmEndpoint { get; set; } = "http://localhost:11434/v1/chat/completions";
    public string LlmModel { get; set; } = "qwen2.5:7b";
    public string LlmApiKeyEncrypted { get; set; } = string.Empty;

    public static readonly string AppDataFolder = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
        "QuickTranslateVN");

    private static readonly string SettingsPath = Path.Combine(AppDataFolder, "settings.json");
    private const string StartupRegistryKey = @"Software\Microsoft\Windows\CurrentVersion\Run";
    private const string AppRegistryName = "QuickTranslateVN";

    [JsonIgnore]
    public string DeepLApiKey
    {
        get => DecryptSecret(DeepLApiKeyEncrypted);
        set => DeepLApiKeyEncrypted = EncryptSecret(value);
    }

    [JsonIgnore]
    public string LlmApiKey
    {
        get => DecryptSecret(LlmApiKeyEncrypted);
        set => LlmApiKeyEncrypted = EncryptSecret(value);
    }

    public static AppSettings Load()
    {
        try
        {
            if (File.Exists(SettingsPath))
            {
                var json = File.ReadAllText(SettingsPath);
                return JsonSerializer.Deserialize<AppSettings>(json) ?? new AppSettings();
            }
        }
        catch
        {
            // Return defaults if config is unreadable
        }
        return new AppSettings();
    }

    public void Save()
    {
        try
        {
            Directory.CreateDirectory(AppDataFolder);
            var json = JsonSerializer.Serialize(this, new JsonSerializerOptions { WriteIndented = true });
            File.WriteAllText(SettingsPath, json);
        }
        catch
        {
            // Ignore IO errors
        }
    }

    private static string EncryptSecret(string? plainText)
    {
        if (string.IsNullOrWhiteSpace(plainText)) return string.Empty;
        try
        {
            byte[] bytes = Encoding.UTF8.GetBytes(plainText.Trim());
            byte[] encrypted = ProtectedData.Protect(bytes, null, DataProtectionScope.CurrentUser);
            return Convert.ToBase64String(encrypted);
        }
        catch
        {
            return string.Empty;
        }
    }

    private static string DecryptSecret(string? cipherBase64)
    {
        if (string.IsNullOrWhiteSpace(cipherBase64)) return string.Empty;
        try
        {
            byte[] encrypted = Convert.FromBase64String(cipherBase64);
            byte[] decrypted = ProtectedData.Unprotect(encrypted, null, DataProtectionScope.CurrentUser);
            return Encoding.UTF8.GetString(decrypted);
        }
        catch
        {
            return string.Empty;
        }
    }

    public static bool IsStartupEnabled()
    {
        try
        {
            using var key = Registry.CurrentUser.OpenSubKey(StartupRegistryKey, false);
            return key?.GetValue(AppRegistryName) != null;
        }
        catch
        {
            return false;
        }
    }

    public static void SetStartupEnabled(bool enable)
    {
        try
        {
            using var key = Registry.CurrentUser.OpenSubKey(StartupRegistryKey, true);
            if (key == null) return;

            if (enable)
            {
                var exePath = Environment.ProcessPath;
                if (!string.IsNullOrEmpty(exePath))
                    key.SetValue(AppRegistryName, $"\"{exePath}\"");
            }
            else
            {
                key.DeleteValue(AppRegistryName, false);
            }
        }
        catch
        {
            // Ignore registry errors
        }
    }
}
