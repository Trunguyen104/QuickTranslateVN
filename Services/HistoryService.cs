using System.IO;
using System.Text.Json;

namespace QuickTranslateVN.Services;

public sealed class HistoryItem
{
    public DateTime Timestamp { get; init; } = DateTime.Now;
    public string SourceText { get; init; } = string.Empty;
    public string TranslatedText { get; init; } = string.Empty;
    public string LangPair { get; init; } = "EN → VI";
    public string Provider { get; init; } = "Google";

    public string TimeDisplay => Timestamp.ToString("HH:mm dd/MM");
}

public sealed class HistoryService
{
    private const int MaxEntries = 80;
    private static readonly string HistoryFilePath = Path.Combine(AppSettings.AppDataFolder, "history.json");
    private readonly List<HistoryItem> _items = [];
    private readonly object _lock = new();

    public HistoryService()
    {
        Load();
    }

    public IReadOnlyList<HistoryItem> GetItems()
    {
        lock (_lock)
        {
            return _items.ToList();
        }
    }

    public void Record(string source, string translated, string srcLang, string tgtLang, string provider, bool saveEnabled)
    {
        if (!saveEnabled || string.IsNullOrWhiteSpace(source) || string.IsNullOrWhiteSpace(translated))
            return;

        lock (_lock)
        {
            // Deduplicate consecutive identical lookups
            _items.RemoveAll(x => string.Equals(x.SourceText, source, StringComparison.OrdinalIgnoreCase)
                               && string.Equals(x.TranslatedText, translated, StringComparison.OrdinalIgnoreCase));

            _items.Insert(0, new HistoryItem
            {
                Timestamp = DateTime.Now,
                SourceText = source,
                TranslatedText = translated,
                LangPair = $"{srcLang.ToUpperInvariant()} → {tgtLang.ToUpperInvariant()}",
                Provider = provider
            });

            if (_items.Count > MaxEntries)
                _items.RemoveRange(MaxEntries, _items.Count - MaxEntries);

            Save();
        }
    }

    public void Clear()
    {
        lock (_lock)
        {
            _items.Clear();
            try
            {
                if (File.Exists(HistoryFilePath))
                    File.Delete(HistoryFilePath);
            }
            catch
            {
                // Ignore delete error
            }
        }
    }

    private void Load()
    {
        try
        {
            if (File.Exists(HistoryFilePath))
            {
                var json = File.ReadAllText(HistoryFilePath);
                var loaded = JsonSerializer.Deserialize<List<HistoryItem>>(json);
                if (loaded != null)
                    _items.AddRange(loaded.Take(MaxEntries));
            }
        }
        catch
        {
            // Ignore unreadable history
        }
    }

    private void Save()
    {
        try
        {
            Directory.CreateDirectory(AppSettings.AppDataFolder);
            var json = JsonSerializer.Serialize(_items);
            File.WriteAllText(HistoryFilePath, json);
        }
        catch
        {
            // Ignore IO errors
        }
    }
}
