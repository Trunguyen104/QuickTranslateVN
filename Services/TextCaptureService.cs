using System.Collections.Concurrent;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Windows.Automation;
using QuickTranslateVN.Interop;

namespace QuickTranslateVN.Services;

public sealed class CapturedSelection
{
    public string Text { get; init; } = string.Empty;
    public string Method { get; init; } = string.Empty;
    public NativeMethods.POINT AnchorPoint { get; init; }
}

public sealed class TextCaptureService : IDisposable
{
    private static readonly HashSet<string> TerminalProcesses = new(StringComparer.OrdinalIgnoreCase)
    {
        "cmd", "powershell", "pwsh", "WindowsTerminal", "mintty", "wsl", "alacritty", "wezterm-gui", "conhost"
    };

    // Single reusable background STA worker thread — zero per-call thread allocations!
    private readonly BlockingCollection<Action> _staQueue = new();
    private readonly Thread _staWorkerThread;

    public TextCaptureService()
    {
        NativeMethods.RegisterClipboardFormat("ExcludeClipboardContentFromMonitorProcessing");
        NativeMethods.RegisterClipboardFormat("CanIncludeInClipboardHistory");
        NativeMethods.RegisterClipboardFormat("CanUploadToCloudClipboard");

        _staWorkerThread = new Thread(StaWorkerLoop)
        {
            Name = "QuickTranslateVN_UIA_STA_Worker",
            IsBackground = true
        };
        _staWorkerThread.SetApartmentState(ApartmentState.STA);
        _staWorkerThread.Start();
    }

    private void StaWorkerLoop()
    {
        foreach (var workItem in _staQueue.GetConsumingEnumerable())
        {
            try
            {
                workItem();
            }
            catch
            {
                // Swallow COM exceptions inside worker loop
            }
        }
    }

    public async Task<CapturedSelection?> ReadCurrentClipboardSelectionAsync()
    {
        await Task.Delay(45);
        NativeMethods.GetCursorPos(out var pt);

        string text = string.Empty;
        await System.Windows.Application.Current.Dispatcher.InvokeAsync(() =>
        {
            try
            {
                if (System.Windows.Clipboard.ContainsText())
                    text = System.Windows.Clipboard.GetText();
            }
            catch
            {
                // Ignore transient clipboard lock
            }
        });

        if (string.IsNullOrWhiteSpace(text))
            return null;

        return new CapturedSelection
        {
            Text = text.Trim(),
            Method = "Double Ctrl+C",
            AnchorPoint = pt
        };
    }

    public async Task<CapturedSelection?> CaptureSelectedTextAsync(NativeMethods.POINT? preferredAnchor = null)
    {
        NativeMethods.GetCursorPos(out var cursorPt);
        var anchor = preferredAnchor ?? cursorPt;
        string? fgProcess = GetForegroundProcessName();

        // --- TIER 1: Windows UI Automation (TextPattern + Excel ValuePattern) ---
        var uiaResult = await TryCaptureViaUiaAsync(fgProcess, TimeSpan.FromMilliseconds(180));
        if (uiaResult != null && !string.IsNullOrWhiteSpace(uiaResult.Text))
        {
            return new CapturedSelection
            {
                Text = uiaResult.Text.Trim(),
                Method = uiaResult.Method,
                AnchorPoint = preferredAnchor ?? uiaResult.AnchorPoint
            };
        }

        // --- Terminal Guard ---
        if (fgProcess != null && TerminalProcesses.Contains(fgProcess))
        {
            return null;
        }

        // --- TIER 2: Safe Clipboard Simulation ---
        var clipText = await TryCaptureViaSafeClipboardAsync();
        if (!string.IsNullOrWhiteSpace(clipText))
        {
            return new CapturedSelection
            {
                Text = clipText.TrimEnd('\r', '\n', '\t').Trim(),
                Method = "Clipboard",
                AnchorPoint = anchor
            };
        }

        return null;
    }

    /// <summary>
    /// Replaces currently highlighted text inside the active foreground editor with translated text
    /// (inspired by Trayslate & DeepL for Windows), while preserving user's original Clipboard!
    /// </summary>
    public async Task<bool> ReplaceSelectedTextInActiveAppAsync(string replacementText)
    {
        if (string.IsNullOrEmpty(replacementText))
            return false;

        string? fgProcess = GetForegroundProcessName();
        if (fgProcess != null && TerminalProcesses.Contains(fgProcess))
            return false;

        var (backupFormats, backupPlainText) = await BackupClipboardAsync();

        try
        {
            await System.Windows.Application.Current.Dispatcher.InvokeAsync(() =>
            {
                var dataObj = new System.Windows.DataObject();
                dataObj.SetText(replacementText);
                dataObj.SetData("ExcludeClipboardContentFromMonitorProcessing", 0);
                dataObj.SetData("CanIncludeInClipboardHistory", 0);
                dataObj.SetData("CanUploadToCloudClipboard", 0);
                System.Windows.Clipboard.SetDataObject(dataObj, true);
            });

            await Task.Delay(25);
            SendCleanKeyCombo(NativeMethods.VK_V);
            await Task.Delay(75);
            return true;
        }
        finally
        {
            await System.Windows.Application.Current.Dispatcher.InvokeAsync(() =>
            {
                RestoreClipboardSilently(backupFormats, backupPlainText);
            });
        }
    }

    private async Task<CapturedSelection?> TryCaptureViaUiaAsync(string? fgProcess, TimeSpan timeout)
    {
        // Guard against queue buildup if a previous target window froze UIA
        if (_staQueue.Count > 1)
            return null;

        var tcs = new TaskCompletionSource<CapturedSelection?>(TaskCreationOptions.RunContinuationsAsynchronously);

        _staQueue.Add(() =>
        {
            try
            {
                var element = AutomationElement.FocusedElement;
                if (element == null)
                {
                    tcs.TrySetResult(null);
                    return;
                }

                // 1. Standard TextPattern + Parent Walk (Word, Notepad, WPF, Windows Terminal, Browsers)
                TextPattern? textPattern = null;
                var current = element;
                var walker = TreeWalker.ControlViewWalker;

                for (int depth = 0; depth < 5 && current != null; depth++)
                {
                    if (current.TryGetCurrentPattern(TextPattern.Pattern, out object? patternObj) &&
                        patternObj is TextPattern tp)
                    {
                        textPattern = tp;
                        break;
                    }
                    current = walker.GetParent(current);
                }

                if (textPattern != null)
                {
                    var selections = textPattern.GetSelection();
                    if (selections is { Length: > 0 })
                    {
                        var sb = new System.Text.StringBuilder();
                        NativeMethods.GetCursorPos(out var anchor);

                        foreach (var range in selections)
                        {
                            sb.Append(range.GetText(6000));
                            var rects = range.GetBoundingRectangles();
                            if (rects is { Length: > 0 })
                            {
                                var lastRect = rects[^1];
                                if (!lastRect.IsEmpty && lastRect.Width > 0 && lastRect.Height > 0)
                                {
                                    anchor = new NativeMethods.POINT
                                    {
                                        X = (int)(lastRect.Left + Math.Min(lastRect.Width / 2, 120)),
                                        Y = (int)lastRect.Bottom
                                    };
                                }
                            }
                        }

                        var extracted = sb.ToString();
                        if (!string.IsNullOrWhiteSpace(extracted))
                        {
                            tcs.TrySetResult(new CapturedSelection
                            {
                                Text = extracted,
                                Method = "UI Automation",
                                AnchorPoint = anchor
                            });
                            return;
                        }
                    }
                }

                // 2. Excel Cell Mode Fast-Path (reads cell value directly without Excel Ctrl+C marching ants!)
                if (string.Equals(fgProcess, "EXCEL", StringComparison.OrdinalIgnoreCase) &&
                    element.TryGetCurrentPattern(ValuePattern.Pattern, out object? valPatternObj) &&
                    valPatternObj is ValuePattern vp)
                {
                    var cellText = vp.Current.Value;
                    if (!string.IsNullOrWhiteSpace(cellText))
                    {
                        NativeMethods.GetCursorPos(out var pt);
                        tcs.TrySetResult(new CapturedSelection
                        {
                            Text = cellText,
                            Method = "Excel UIA",
                            AnchorPoint = pt
                        });
                        return;
                    }
                }

                tcs.TrySetResult(null);
            }
            catch
            {
                tcs.TrySetResult(null);
            }
        });

        var completed = await Task.WhenAny(tcs.Task, Task.Delay(timeout));
        return completed == tcs.Task ? await tcs.Task : null;
    }

    private static string? GetForegroundProcessName()
    {
        try
        {
            IntPtr hwnd = NativeMethods.GetForegroundWindow();
            if (hwnd == IntPtr.Zero) return null;
            NativeMethods.GetWindowThreadProcessId(hwnd, out uint pid);
            if (pid == 0) return null;
            using var proc = Process.GetProcessById((int)pid);
            return proc.ProcessName;
        }
        catch
        {
            return null;
        }
    }

    private static async Task<(Dictionary<string, object>? Formats, string? PlainText)> BackupClipboardAsync()
    {
        Dictionary<string, object>? backupFormats = null;
        string? backupPlainText = null;

        await System.Windows.Application.Current.Dispatcher.InvokeAsync(() =>
        {
            try
            {
                if (System.Windows.Clipboard.ContainsText())
                    backupPlainText = System.Windows.Clipboard.GetText();

                var dataObj = System.Windows.Clipboard.GetDataObject();
                if (dataObj != null)
                {
                    backupFormats = new Dictionary<string, object>();
                    foreach (var fmt in dataObj.GetFormats(false))
                    {
                        try
                        {
                            var data = dataObj.GetData(fmt, false);
                            if (data != null)
                                backupFormats[fmt] = data;
                        }
                        catch
                        {
                            // Skip non-serializable DelayedRender formats
                        }
                    }
                }
            }
            catch
            {
                // Ignore backup errors
            }
        });

        return (backupFormats, backupPlainText);
    }

    private async Task<string?> TryCaptureViaSafeClipboardAsync()
    {
        var (backupFormats, backupPlainText) = await BackupClipboardAsync();

        // Wait up to ~180ms if user is still holding Alt/Shift from hotkey
        for (int i = 0; i < 12; i++)
        {
            bool altHeld = (NativeMethods.GetAsyncKeyState(NativeMethods.VK_MENU) & 0x8000) != 0;
            bool shiftHeld = (NativeMethods.GetAsyncKeyState(NativeMethods.VK_SHIFT) & 0x8000) != 0;
            if (!altHeld && !shiftHeld) break;
            await Task.Delay(15);
        }

        uint seqBefore = NativeMethods.GetClipboardSequenceNumber();
        SendCleanKeyCombo(NativeMethods.VK_C);

        bool changed = false;
        for (int i = 0; i < 23; i++)
        {
            await Task.Delay(12);
            if (NativeMethods.GetClipboardSequenceNumber() != seqBefore)
            {
                changed = true;
                break;
            }
        }

        if (!changed)
            return null;

        await Task.Delay(15);

        string? captured = null;
        await System.Windows.Application.Current.Dispatcher.InvokeAsync(() =>
        {
            for (int retry = 0; retry < 4; retry++)
            {
                try
                {
                    if (System.Windows.Clipboard.ContainsText())
                        captured = System.Windows.Clipboard.GetText();
                    break;
                }
                catch
                {
                    Thread.Sleep(12);
                }
            }
        });

        await System.Windows.Application.Current.Dispatcher.InvokeAsync(() =>
        {
            RestoreClipboardSilently(backupFormats, backupPlainText);
        });

        return captured;
    }

    private static void SendCleanKeyCombo(ushort vkLetter)
    {
        var inputs = new List<NativeMethods.INPUT>();

        if ((NativeMethods.GetAsyncKeyState(NativeMethods.VK_MENU) & 0x8000) != 0)
            inputs.Add(CreateKeyInput(NativeMethods.VK_MENU, keyUp: true));
        if ((NativeMethods.GetAsyncKeyState(NativeMethods.VK_SHIFT) & 0x8000) != 0)
            inputs.Add(CreateKeyInput(NativeMethods.VK_SHIFT, keyUp: true));

        inputs.Add(CreateKeyInput(NativeMethods.VK_CONTROL, keyUp: false));
        inputs.Add(CreateKeyInput(vkLetter, keyUp: false));
        inputs.Add(CreateKeyInput(vkLetter, keyUp: true));
        inputs.Add(CreateKeyInput(NativeMethods.VK_CONTROL, keyUp: true));

        var arr = inputs.ToArray();
        NativeMethods.SendInput((uint)arr.Length, arr, Marshal.SizeOf<NativeMethods.INPUT>());
    }

    private static NativeMethods.INPUT CreateKeyInput(ushort vk, bool keyUp)
    {
        return new NativeMethods.INPUT
        {
            type = NativeMethods.INPUT_KEYBOARD,
            U = new NativeMethods.InputUnion
            {
                ki = new NativeMethods.KEYBDINPUT
                {
                    wVk = vk,
                    wScan = 0,
                    dwFlags = keyUp ? NativeMethods.KEYEVENTF_KEYUP : 0,
                    time = 0,
                    dwExtraInfo = IntPtr.Zero
                }
            }
        };
    }

    private static void RestoreClipboardSilently(Dictionary<string, object>? backupFormats, string? backupPlainText)
    {
        try
        {
            if ((backupFormats == null || backupFormats.Count == 0) && string.IsNullOrEmpty(backupPlainText))
            {
                if (NativeMethods.OpenClipboard(IntPtr.Zero))
                {
                    NativeMethods.EmptyClipboard();
                    NativeMethods.CloseClipboard();
                }
                return;
            }

            var dataObject = new System.Windows.DataObject();

            if (backupFormats is { Count: > 0 })
            {
                foreach (var kvp in backupFormats)
                {
                    try
                    {
                        dataObject.SetData(kvp.Key, kvp.Value);
                    }
                    catch
                    {
                        // Ignore format restore error
                    }
                }
            }
            else if (!string.IsNullOrEmpty(backupPlainText))
            {
                dataObject.SetText(backupPlainText);
            }

            dataObject.SetData("ExcludeClipboardContentFromMonitorProcessing", 0);
            dataObject.SetData("CanIncludeInClipboardHistory", 0);
            dataObject.SetData("CanUploadToCloudClipboard", 0);

            for (int retry = 0; retry < 4; retry++)
            {
                try
                {
                    System.Windows.Clipboard.SetDataObject(dataObject, true);
                    break;
                }
                catch
                {
                    Thread.Sleep(12);
                }
            }
        }
        catch
        {
            // Best-effort restore
        }
    }

    public void Dispose()
    {
        _staQueue.CompleteAdding();
        _staQueue.Dispose();
    }
}
