using System.Collections.Concurrent;
using System.IO;
using System.Net.Http;
using System.Text;
using System.Text.Json;
using System.Windows.Media;

namespace QuickTranslateVN.Services;

public sealed class DictionaryEntryItem
{
    public string PartOfSpeech { get; init; } = string.Empty;
    public List<string> Meanings { get; init; } = [];
}

public sealed class TranslationResultModel
{
    public string OriginalText { get; init; } = string.Empty;
    public string TranslatedText { get; init; } = string.Empty;
    public string SourceLang { get; init; } = "en";
    public string TargetLang { get; init; } = "vi";
    public string? Phonetic { get; init; }
    public List<DictionaryEntryItem> DictionaryEntries { get; init; } = [];
    public long ElapsedMs { get; init; }
}

/// <summary>
/// Zero-dependency ultra-light translation engine using pure .NET HttpClient.
/// </summary>
public sealed class TranslationService : IDisposable
{
    private readonly HttpClient _httpClient;
    private readonly ConcurrentDictionary<string, TranslationResultModel> _cache = new();
    private MediaPlayer? _mediaPlayer;
    private string? _lastTempAudioFile;

    public TranslationService()
    {
        _httpClient = new HttpClient(new SocketsHttpHandler
        {
            PooledConnectionLifetime = TimeSpan.FromMinutes(5)
        })
        {
            Timeout = TimeSpan.FromSeconds(8)
        };
        _httpClient.DefaultRequestHeaders.UserAgent.ParseAdd(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36");
    }

    public async Task<TranslationResultModel> TranslateAsync(
        string text,
        string? forceTargetLang = null,
        CancellationToken ct = default)
    {
        var trimmed = text.Trim();
        if (trimmed.Length > 4500)
            trimmed = trimmed[..4500];

        string targetLang = forceTargetLang ?? (TextCleaner.LooksLikeVietnamese(trimmed) ? "en" : "vi");
        string cacheKey = $"{targetLang}::{trimmed}";

        if (_cache.TryGetValue(cacheKey, out var cached))
            return cached;

        var sw = System.Diagnostics.Stopwatch.StartNew();
        TranslationResultModel result;

        try
        {
            result = await QueryGoogleEndpointAsync("gtx", trimmed, targetLang, ct);
        }
        catch when (!ct.IsCancellationRequested)
        {
            // Built-in backup client if 'gtx' ever throttles
            result = await QueryGoogleEndpointAsync("dict-chrome-ex", trimmed, targetLang, ct);
        }

        sw.Stop();
        var finalModel = new TranslationResultModel
        {
            OriginalText = trimmed,
            TranslatedText = result.TranslatedText,
            SourceLang = result.SourceLang,
            TargetLang = result.TargetLang,
            Phonetic = result.Phonetic,
            DictionaryEntries = result.DictionaryEntries,
            ElapsedMs = sw.ElapsedMilliseconds
        };

        if (_cache.Count > 80)
            _cache.Clear();
        _cache[cacheKey] = finalModel;

        return finalModel;
    }

    private async Task<TranslationResultModel> QueryGoogleEndpointAsync(
        string clientName,
        string text,
        string targetLang,
        CancellationToken ct,
        bool allowAutoFlip = true)
    {
        string url =
            $"https://translate.googleapis.com/translate_a/single?client={clientName}&sl=auto&tl={targetLang}&hl=vi&dt=t&dt=bd&dt=rm&dj=1&q={Uri.EscapeDataString(text)}";

        using var response = await _httpClient.GetAsync(url, ct);
        response.EnsureSuccessStatusCode();

        using var stream = await response.Content.ReadAsStreamAsync(ct);
        using var doc = await JsonDocument.ParseAsync(stream, cancellationToken: ct);
        var root = doc.RootElement;

        var sbTrans = new StringBuilder();
        string? phonetic = null;

        if (root.TryGetProperty("sentences", out var sentences) && sentences.ValueKind == JsonValueKind.Array)
        {
            foreach (var item in sentences.EnumerateArray())
            {
                if (item.TryGetProperty("trans", out var transProp) && transProp.ValueKind == JsonValueKind.String)
                    sbTrans.Append(transProp.GetString());

                if (phonetic == null && item.TryGetProperty("src_translit", out var translitProp) &&
                    translitProp.ValueKind == JsonValueKind.String)
                {
                    var val = translitProp.GetString();
                    if (!string.IsNullOrWhiteSpace(val))
                        phonetic = val;
                }
            }
        }

        string detectedSrc = root.TryGetProperty("src", out var srcProp) && srcProp.ValueKind == JsonValueKind.String
            ? (srcProp.GetString() ?? "en")
            : "en";

        if (allowAutoFlip && string.Equals(detectedSrc, targetLang, StringComparison.OrdinalIgnoreCase) && targetLang == "vi")
        {
            return await QueryGoogleEndpointAsync(clientName, text, "en", ct, allowAutoFlip: false);
        }

        var dictList = new List<DictionaryEntryItem>();
        if (root.TryGetProperty("dict", out var dictArr) && dictArr.ValueKind == JsonValueKind.Array)
        {
            foreach (var dictEntry in dictArr.EnumerateArray())
            {
                string pos = dictEntry.TryGetProperty("pos", out var posProp) ? (posProp.GetString() ?? "") : "";
                var terms = new List<string>();

                if (dictEntry.TryGetProperty("terms", out var termsArr) && termsArr.ValueKind == JsonValueKind.Array)
                {
                    foreach (var t in termsArr.EnumerateArray())
                    {
                        var termStr = t.GetString();
                        if (!string.IsNullOrWhiteSpace(termStr) && terms.Count < 5)
                            terms.Add(termStr);
                    }
                }

                if (terms.Count > 0)
                    dictList.Add(new DictionaryEntryItem { PartOfSpeech = pos, Meanings = terms });
            }
        }

        var translated = sbTrans.ToString().Trim();
        if (string.IsNullOrEmpty(translated))
            throw new InvalidOperationException("Empty response.");

        return new TranslationResultModel
        {
            OriginalText = text,
            TranslatedText = translated,
            SourceLang = detectedSrc,
            TargetLang = targetLang,
            Phonetic = phonetic,
            DictionaryEntries = dictList
        };
    }

    public async Task SpeakAsync(string text, string langCode)
    {
        if (string.IsNullOrWhiteSpace(text)) return;
        try
        {
            var snippet = text.Trim();
            if (snippet.Length > 200) snippet = snippet[..200];

            string tl = string.IsNullOrWhiteSpace(langCode) ? "en" : langCode;
            string url =
                $"https://translate.googleapis.com/translate_tts?ie=UTF-8&client=tw-ob&tl={Uri.EscapeDataString(tl)}&q={Uri.EscapeDataString(snippet)}";

            byte[] audioBytes = await _httpClient.GetByteArrayAsync(url);
            string tempFile = Path.Combine(Path.GetTempPath(), $"qtvn_{Guid.NewGuid():N}.mp3");
            await File.WriteAllBytesAsync(tempFile, audioBytes);

            await System.Windows.Application.Current.Dispatcher.InvokeAsync(() =>
            {
                CleanupTempAudio();
                _lastTempAudioFile = tempFile;
                _mediaPlayer = new MediaPlayer();
                _mediaPlayer.Open(new Uri(tempFile));
                _mediaPlayer.Play();
            });
        }
        catch { }
    }

    public void CleanupTempAudio()
    {
        try
        {
            _mediaPlayer?.Close();
            _mediaPlayer = null;
            if (!string.IsNullOrEmpty(_lastTempAudioFile) && File.Exists(_lastTempAudioFile))
                File.Delete(_lastTempAudioFile);
        }
        catch { }
    }

    public void Dispose()
    {
        CleanupTempAudio();
        _httpClient.Dispose();
    }
}
