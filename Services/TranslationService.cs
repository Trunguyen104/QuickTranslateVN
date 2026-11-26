using System.Collections.Concurrent;
using System.IO;
using System.Net.Http;
using System.Net.Http.Headers;
using System.Text;
using System.Text.Json;
using System.Windows.Media;
using GTranslate.Translators;

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
    public string? BackTranslatedText { get; set; }
    public string SourceLang { get; init; } = "en";
    public string TargetLang { get; init; } = "vi";
    public string? Phonetic { get; init; }
    public List<DictionaryEntryItem> DictionaryEntries { get; init; } = [];
    public string EngineName { get; init; } = "Google";
    public long ElapsedMs { get; init; }
}

public sealed class TranslationService : IDisposable
{
    private readonly HttpClient _httpClient;
    private readonly BingTranslator _bingTranslator;
    private readonly AggregateTranslator _fallbackTranslator;
    private readonly ConcurrentDictionary<string, TranslationResultModel> _cache = new();
    private MediaPlayer? _mediaPlayer;
    private string? _lastTempAudioFile;

    public TranslationService()
    {
        _httpClient = new HttpClient
        {
            Timeout = TimeSpan.FromSeconds(18)
        };
        _httpClient.DefaultRequestHeaders.UserAgent.ParseAdd(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36");

        _bingTranslator = new BingTranslator();
        _fallbackTranslator = new AggregateTranslator();
    }

    public async Task<TranslationResultModel> TranslateAsync(
        string text,
        AppSettings settings,
        string? forceTargetLang = null,
        CancellationToken ct = default)
    {
        var trimmed = text.Trim();
        if (trimmed.Length > 5000)
            trimmed = trimmed[..5000];

        bool looksVn = TextCleaner.LooksLikeVietnamese(trimmed);
        string targetLang = forceTargetLang ?? (looksVn ? "en" : "vi");
        string cacheKey = $"{settings.Provider}::{targetLang}::{trimmed}";

        if (_cache.TryGetValue(cacheKey, out var cached))
        {
            if (settings.EnableBackTranslation && string.IsNullOrEmpty(cached.BackTranslatedText))
            {
                cached.BackTranslatedText = await TryTranslateBackAsync(cached.TranslatedText, cached.SourceLang, ct);
            }
            return cached;
        }

        var sw = System.Diagnostics.Stopwatch.StartNew();
        TranslationResultModel result;

        try
        {
            result = settings.Provider switch
            {
                TranslationProviderType.MicrosoftFree => await TranslateViaMicrosoftFreeAsync(trimmed, targetLang),
                TranslationProviderType.DeepL => await TranslateViaDeepLAsync(trimmed, targetLang, settings.DeepLApiKey, ct),
                TranslationProviderType.OpenAiCompatible => await TranslateViaLlmAsync(trimmed, looksVn ? "vi" : "en", targetLang, settings, ct),
                _ => await TranslateViaGoogleGtxAsync(trimmed, targetLang, ct)
            };
        }
        catch when (!ct.IsCancellationRequested && settings.Provider == TranslationProviderType.GoogleFree)
        {
            // Automatic resilient failover to Microsoft/Bing/Google2 if Google GTX temporarily throttles
            var fb = await _fallbackTranslator.TranslateAsync(trimmed, targetLang);
            result = new TranslationResultModel
            {
                OriginalText = trimmed,
                TranslatedText = fb.Translation,
                SourceLang = fb.SourceLanguage.ISO6391,
                TargetLang = fb.TargetLanguage.ISO6391,
                EngineName = fb.Service
            };
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
            EngineName = result.EngineName,
            ElapsedMs = sw.ElapsedMilliseconds
        };

        if (settings.EnableBackTranslation)
        {
            finalModel.BackTranslatedText = await TryTranslateBackAsync(finalModel.TranslatedText, finalModel.SourceLang, ct);
        }

        StoreInCache(cacheKey, finalModel);
        return finalModel;
    }

    public async Task<string?> TryTranslateBackAsync(string translatedText, string originalSourceLang, CancellationToken ct)
    {
        try
        {
            string backLang = string.IsNullOrWhiteSpace(originalSourceLang) || originalSourceLang == "auto" ? "en" : originalSourceLang;
            var back = await TranslateViaGoogleGtxAsync(translatedText, backLang, ct, allowAutoFlip: false);
            return back.TranslatedText;
        }
        catch
        {
            return null;
        }
    }

    private async Task<TranslationResultModel> TranslateViaMicrosoftFreeAsync(string text, string targetLang)
    {
        var res = await _bingTranslator.TranslateAsync(text, targetLang);
        string srcIso = res.SourceLanguage?.ISO6391 ?? "en";
        if (string.Equals(srcIso, targetLang, StringComparison.OrdinalIgnoreCase) && targetLang == "vi")
        {
            res = await _bingTranslator.TranslateAsync(text, "en");
            srcIso = res.SourceLanguage?.ISO6391 ?? "vi";
        }

        return new TranslationResultModel
        {
            OriginalText = text,
            TranslatedText = res.Translation,
            SourceLang = srcIso,
            TargetLang = res.TargetLanguage?.ISO6391 ?? targetLang,
            EngineName = "Microsoft"
        };
    }

    private async Task<TranslationResultModel> TranslateViaDeepLAsync(
        string text,
        string targetLang,
        string apiKey,
        CancellationToken ct)
    {
        if (string.IsNullOrWhiteSpace(apiKey))
            throw new InvalidOperationException("Chưa nhập DeepL API Key trong mục Cài đặt (⚙️).");

        bool isFreeKey = apiKey.Trim().EndsWith(":fx", StringComparison.OrdinalIgnoreCase);
        string endpoint = isFreeKey
            ? "https://api-free.deepl.com/v2/translate"
            : "https://api.deepl.com/v2/translate";

        string dlTarget = targetLang.Equals("en", StringComparison.OrdinalIgnoreCase) ? "EN-US" : targetLang.ToUpperInvariant();

        using var req = new HttpRequestMessage(HttpMethod.Post, endpoint);
        req.Headers.Authorization = new AuthenticationHeaderValue("DeepL-Auth-Key", apiKey.Trim());
        var payload = JsonSerializer.Serialize(new
        {
            text = new[] { text },
            target_lang = dlTarget
        });
        req.Content = new StringContent(payload, Encoding.UTF8, "application/json");

        using var resp = await _httpClient.SendAsync(req, ct);
        resp.EnsureSuccessStatusCode();

        var json = await resp.Content.ReadAsStringAsync(ct);
        using var doc = JsonDocument.Parse(json);
        var first = doc.RootElement.GetProperty("translations")[0];
        string trans = first.GetProperty("text").GetString() ?? "";
        string src = first.TryGetProperty("detected_source_language", out var s) ? (s.GetString()?.ToLowerInvariant() ?? "en") : "en";

        return new TranslationResultModel
        {
            OriginalText = text,
            TranslatedText = trans,
            SourceLang = src,
            TargetLang = targetLang,
            EngineName = "DeepL"
        };
    }

    private async Task<TranslationResultModel> TranslateViaLlmAsync(
        string text,
        string guessedSourceLang,
        string targetLang,
        AppSettings settings,
        CancellationToken ct)
    {
        string endpoint = string.IsNullOrWhiteSpace(settings.LlmEndpoint)
            ? "http://localhost:11434/v1/chat/completions"
            : settings.LlmEndpoint.Trim();

        string targetName = targetLang == "vi" ? "Vietnamese" : "English";
        string systemPrompt =
            $"You are a professional translator. Translate the user's text accurately and naturally into {targetName}. " +
            "Preserve technical terms appropriately and output ONLY the translated text without explanations or quotes.";

        using var req = new HttpRequestMessage(HttpMethod.Post, endpoint);
        if (!string.IsNullOrWhiteSpace(settings.LlmApiKey))
        {
            req.Headers.Authorization = new AuthenticationHeaderValue("Bearer", settings.LlmApiKey.Trim());
        }

        var body = JsonSerializer.Serialize(new
        {
            model = string.IsNullOrWhiteSpace(settings.LlmModel) ? "qwen2.5:7b" : settings.LlmModel.Trim(),
            temperature = 0.2,
            messages = new[]
            {
                new { role = "system", content = systemPrompt },
                new { role = "user", content = text }
            }
        });
        req.Content = new StringContent(body, Encoding.UTF8, "application/json");

        using var resp = await _httpClient.SendAsync(req, ct);
        resp.EnsureSuccessStatusCode();

        var json = await resp.Content.ReadAsStringAsync(ct);
        using var doc = JsonDocument.Parse(json);
        string content = doc.RootElement
            .GetProperty("choices")[0]
            .GetProperty("message")
            .GetProperty("content")
            .GetString()?.Trim() ?? "";

        bool isLocal = endpoint.Contains("localhost", StringComparison.OrdinalIgnoreCase) ||
                       endpoint.Contains("127.0.0.1", StringComparison.OrdinalIgnoreCase);

        return new TranslationResultModel
        {
            OriginalText = text,
            TranslatedText = content,
            SourceLang = guessedSourceLang,
            TargetLang = targetLang,
            EngineName = isLocal ? "Local AI (Offline)" : "AI LLM"
        };
    }

    private async Task<TranslationResultModel> TranslateViaGoogleGtxAsync(
        string text,
        string targetLang,
        CancellationToken ct,
        bool allowAutoFlip = true)
    {
        string encoded = Uri.EscapeDataString(text);
        string url =
            $"https://translate.googleapis.com/translate_a/single?client=gtx&sl=auto&tl={targetLang}&hl=vi&dt=t&dt=bd&dt=rm&dj=1&q={encoded}";

        using var response = await _httpClient.GetAsync(url, ct);
        response.EnsureSuccessStatusCode();

        var json = await response.Content.ReadAsStringAsync(ct);
        using var doc = JsonDocument.Parse(json);
        var root = doc.RootElement;

        var sbTrans = new StringBuilder();
        string? phonetic = null;

        if (root.TryGetProperty("sentences", out var sentences) && sentences.ValueKind == JsonValueKind.Array)
        {
            foreach (var item in sentences.EnumerateArray())
            {
                if (item.TryGetProperty("trans", out var transProp) && transProp.ValueKind == JsonValueKind.String)
                {
                    sbTrans.Append(transProp.GetString());
                }
                if (phonetic == null && item.TryGetProperty("src_translit", out var translitProp) &&
                    translitProp.ValueKind == JsonValueKind.String)
                {
                    var val = translitProp.GetString();
                    if (!string.IsNullOrWhiteSpace(val))
                        phonetic = val;
                }
            }
        }

        string detectedSrc = "en";
        if (root.TryGetProperty("src", out var srcProp) && srcProp.ValueKind == JsonValueKind.String)
        {
            detectedSrc = srcProp.GetString() ?? "en";
        }

        if (allowAutoFlip && string.Equals(detectedSrc, targetLang, StringComparison.OrdinalIgnoreCase) && targetLang == "vi")
        {
            return await TranslateViaGoogleGtxAsync(text, "en", ct, allowAutoFlip: false);
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
                        if (!string.IsNullOrWhiteSpace(termStr) && terms.Count < 6)
                            terms.Add(termStr);
                    }
                }

                if (terms.Count > 0)
                {
                    dictList.Add(new DictionaryEntryItem
                    {
                        PartOfSpeech = pos,
                        Meanings = terms
                    });
                }
            }
        }

        var translated = sbTrans.ToString().Trim();
        if (string.IsNullOrEmpty(translated))
            throw new InvalidOperationException("Empty translation response.");

        return new TranslationResultModel
        {
            OriginalText = text,
            TranslatedText = translated,
            SourceLang = detectedSrc,
            TargetLang = targetLang,
            Phonetic = phonetic,
            DictionaryEntries = dictList,
            EngineName = "Google"
        };
    }

    public async Task SpeakAsync(string text, string langCode)
    {
        if (string.IsNullOrWhiteSpace(text)) return;

        try
        {
            var snippet = text.Trim();
            if (snippet.Length > 200)
                snippet = snippet[..200];

            string tl = string.IsNullOrWhiteSpace(langCode) ? "en" : langCode;
            string url =
                $"https://translate.googleapis.com/translate_tts?ie=UTF-8&client=tw-ob&tl={Uri.EscapeDataString(tl)}&q={Uri.EscapeDataString(snippet)}";

            byte[] audioBytes = await _httpClient.GetByteArrayAsync(url);

            string tempFile = Path.Combine(Path.GetTempPath(), $"qtvn_tts_{Guid.NewGuid():N}.mp3");
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
        catch
        {
            // Ignore TTS playback network issues silently
        }
    }

    public void CleanupTempAudio()
    {
        try
        {
            _mediaPlayer?.Close();
            _mediaPlayer = null;
            if (!string.IsNullOrEmpty(_lastTempAudioFile) && File.Exists(_lastTempAudioFile))
            {
                File.Delete(_lastTempAudioFile);
            }
        }
        catch
        {
            // Ignore file lock
        }
    }

    public void ClearCache() => _cache.Clear();

    private void StoreInCache(string key, TranslationResultModel value)
    {
        if (_cache.Count > 150)
            _cache.Clear();
        _cache[key] = value;
    }

    public void Dispose()
    {
        CleanupTempAudio();
        _httpClient.Dispose();
        _bingTranslator.Dispose();
        _fallbackTranslator.Dispose();
    }
}
