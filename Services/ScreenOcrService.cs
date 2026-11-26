using System.Drawing;
using System.Drawing.Imaging;
using System.IO;
using Windows.Graphics.Imaging;
using Windows.Media.Ocr;

namespace QuickTranslateVN.Services;

public sealed class ScreenOcrService
{
    /// <summary>
    /// Captures a screen rectangle in physical screen pixels and extracts text using built-in Windows 10/11 OCR.
    /// Zero external dependencies, works offline in ~80-150ms.
    /// </summary>
    public async Task<string?> CaptureAndRecognizeAsync(Rectangle screenRect)
    {
        if (screenRect.Width < 8 || screenRect.Height < 8)
            return null;

        using var bmp = new Bitmap(screenRect.Width, screenRect.Height, PixelFormat.Format32bppArgb);
        using (var g = Graphics.FromImage(bmp))
        {
            g.CopyFromScreen(screenRect.Left, screenRect.Top, 0, 0, screenRect.Size, CopyPixelOperation.SourceCopy);
        }

        using var ms = new MemoryStream();
        bmp.Save(ms, ImageFormat.Png);
        ms.Position = 0;

        var randomAccessStream = ms.AsRandomAccessStream();
        var decoder = await BitmapDecoder.CreateAsync(randomAccessStream);
        using var softwareBitmap = await decoder.GetSoftwareBitmapAsync(
            BitmapPixelFormat.Bgra8,
            BitmapAlphaMode.Premultiplied);

        // Prefer English OCR engine for EN->VI reading; fall back to user profile language if en-US pack isn't present
        var engine = OcrEngine.TryCreateFromLanguage(new Windows.Globalization.Language("en-US"))
                     ?? OcrEngine.TryCreateFromUserProfileLanguages();

        if (engine == null)
            return null;

        var ocrResult = await engine.RecognizeAsync(softwareBitmap);
        if (ocrResult == null || string.IsNullOrWhiteSpace(ocrResult.Text))
            return null;

        // Reconstruct lines cleanly
        var lines = ocrResult.Lines.Select(l => l.Text.Trim()).Where(l => !string.IsNullOrEmpty(l));
        return string.Join("\n", lines);
    }
}
