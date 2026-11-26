using System.Text.RegularExpressions;

namespace QuickTranslateVN.Services;

public static partial class TextCleaner
{
    private static readonly char[] SentenceEndings = ['.', '!', '?', ':', ';', '”', '"', ')', ']'];

    [GeneratedRegex(@"(\p{L})-\s*\r?\n\s*(\p{L})")]
    private static partial Regex HyphenatedBreakRegex();

    [GeneratedRegex(@"[ \t]{2,}")]
    private static partial Regex MultiSpaceRegex();

    /// <summary>
    /// Cleans text copied from PDFs/documents by rejoining broken lines and hyphenated words,
    /// while preserving real paragraph breaks and bullet lists.
    /// </summary>
    public static string Clean(string raw, bool cleanPdfLineBreaks)
    {
        if (string.IsNullOrWhiteSpace(raw))
            return string.Empty;

        // Normalize Unicode ligatures (e.g., 'ﬁ' -> "fi", 'ﬂ' -> "fl")
        var text = raw.Normalize(System.Text.NormalizationForm.FormKC).Trim();

        if (!cleanPdfLineBreaks || !text.Contains('\n'))
            return text;

        // 1. Fix PDF hyphenated line-wraps: "transla-\ntion" -> "translation"
        text = HyphenatedBreakRegex().Replace(text, "$1$2");

        var lines = text.Replace("\r\n", "\n").Split('\n');
        if (lines.Length <= 1)
            return text;

        var sb = new System.Text.StringBuilder(text.Length);
        for (int i = 0; i < lines.Length; i++)
        {
            var current = lines[i].Trim();
            if (string.IsNullOrEmpty(current))
            {
                // Preserve blank lines as paragraph separators
                if (sb.Length > 0 && !sb.ToString().EndsWith("\n\n", StringComparison.Ordinal))
                    sb.Append("\n\n");
                continue;
            }

            sb.Append(current);

            if (i < lines.Length - 1)
            {
                var next = lines[i + 1].Trim();
                if (string.IsNullOrEmpty(next))
                    continue;

                bool isBulletNext = next.StartsWith("- ") || next.StartsWith("• ") ||
                                    next.StartsWith("* ") || (next.Length > 2 && char.IsDigit(next[0]) && (next[1] == '.' || next[1] == ')'));

                char lastChar = current[^1];
                bool endsSentence = Array.IndexOf(SentenceEndings, lastChar) >= 0;

                // Keep newline if it looks like code, bullet list, or short heading; otherwise join with space
                if (isBulletNext || (endsSentence && next.Length > 0 && char.IsUpper(next[0]) && current.Length < 45))
                {
                    sb.Append('\n');
                }
                else
                {
                    sb.Append(' ');
                }
            }
        }

        return MultiSpaceRegex().Replace(sb.ToString(), " ").Trim();
    }

    /// <summary>
    /// Detects if text contains characteristic Vietnamese diacritics so we can auto-flip VI -> EN!
    /// </summary>
    public static bool LooksLikeVietnamese(string text)
    {
        if (string.IsNullOrWhiteSpace(text)) return false;
        const string vnChars = "ăâđêôơưĂÂĐÊÔƠƯáàảãạấầẩẫậắằẳẵặéèẻẽẹếềểễệíìỉĩịóòỏõọốồổỗộớờởỡợúùủũụứừửữựýỳỷỹỵ";
        int count = 0;
        foreach (char c in text)
        {
            if (vnChars.Contains(c))
            {
                count++;
                if (count >= 2 || (text.Length <= 12 && count >= 1))
                    return true;
            }
        }
        return false;
    }
}
