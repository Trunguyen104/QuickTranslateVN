using System.Diagnostics;
using System.Runtime;
using QuickTranslateVN.Interop;

namespace QuickTranslateVN.Services;

/// <summary>
/// Gently trims unused managed heap and Win32 working set a few seconds after UI closes,
/// keeping background System Tray RAM usage very low (~35–75 MB) without hurting responsiveness.
/// </summary>
public static class MemoryOptimizer
{
    private static CancellationTokenSource? _trimCts;

    public static void ScheduleIdleTrim(bool enabled, int delayMs = 3500)
    {
        if (!enabled) return;

        _trimCts?.Cancel();
        _trimCts = new CancellationTokenSource();
        var token = _trimCts.Token;

        _ = Task.Run(async () =>
        {
            try
            {
                await Task.Delay(delayMs, token);
                if (token.IsCancellationRequested) return;

                GCSettings.LargeObjectHeapCompactionMode = GCLargeObjectHeapCompactionMode.CompactOnce;
                GC.Collect(2, GCCollectionMode.Optimized, blocking: false, compacting: true);
                GC.WaitForPendingFinalizers();

                using var proc = Process.GetCurrentProcess();
                NativeMethods.SetProcessWorkingSetSize(proc.Handle, new IntPtr(-1), new IntPtr(-1));
            }
            catch
            {
                // Ignore cancelled or OS permission exceptions
            }
        }, token);
    }
}
