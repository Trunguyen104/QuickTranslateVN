using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Windows.Interop;
using QuickTranslateVN.Interop;

namespace QuickTranslateVN.Services;

public sealed class GlobalInputManager : IDisposable
{
    private const int HOTKEY_TRANSLATE_ID = 9001; // Alt + Q

    private HwndSource? _hwndSource;
    private IntPtr _mouseHookId = IntPtr.Zero;
    private IntPtr _keyboardHookId = IntPtr.Zero;

    private readonly NativeMethods.LowLevelProc _mouseProc;
    private readonly NativeMethods.LowLevelProc _keyboardProc;
    private readonly uint _currentPid;
    private readonly IntPtr _hIBeamCursor;

    private NativeMethods.POINT _mouseDownPt;
    private long _mouseDownTick;
    private bool _mouseDownWasIBeam;
    private bool _isLeftButtonDown;

    private NativeMethods.POINT _lastClickUpPt;
    private long _lastClickUpTick;
    private long _lastCtrlCTick;

    public event Action? QuickTranslateHotkeyPressed;
    public event Action? DoubleCopyTriggered;
    public event Action<NativeMethods.POINT>? TextSelectionSuspected;
    public event Action? ExternalMouseAction;
    public event Action? EscapePressed;

    public bool EnableFloatingBadge { get; set; } = true;
    public bool EnableDoubleCtrlC { get; set; } = true;

    public GlobalInputManager()
    {
        _currentPid = (uint)Environment.ProcessId;
        _hIBeamCursor = NativeMethods.LoadCursor(IntPtr.Zero, NativeMethods.IDC_IBEAM);
        _mouseProc = MouseHookCallback;
        _keyboardProc = KeyboardHookCallback;
    }

    public void Initialize()
    {
        var parameters = new HwndSourceParameters("QuickTranslateVN_HotkeySink")
        {
            Width = 0,
            Height = 0,
            WindowStyle = 0
        };
        _hwndSource = new HwndSource(parameters);
        _hwndSource.AddHook(WndProc);

        NativeMethods.RegisterHotKey(
            _hwndSource.Handle,
            HOTKEY_TRANSLATE_ID,
            NativeMethods.MOD_ALT | NativeMethods.MOD_NOREPEAT,
            NativeMethods.VK_Q);

        using var curProcess = Process.GetCurrentProcess();
        using var curModule = curProcess.MainModule;
        IntPtr hMod = NativeMethods.GetModuleHandle(curModule?.ModuleName);

        _mouseHookId = NativeMethods.SetWindowsHookEx(NativeMethods.WH_MOUSE_LL, _mouseProc, hMod, 0);
        _keyboardHookId = NativeMethods.SetWindowsHookEx(NativeMethods.WH_KEYBOARD_LL, _keyboardProc, hMod, 0);
    }

    private IntPtr WndProc(IntPtr hwnd, int msg, IntPtr wParam, IntPtr lParam, ref bool handled)
    {
        if (msg == NativeMethods.WM_HOTKEY && wParam.ToInt32() == HOTKEY_TRANSLATE_ID)
        {
            handled = true;
            System.Windows.Application.Current?.Dispatcher.BeginInvoke(() => QuickTranslateHotkeyPressed?.Invoke());
        }
        return IntPtr.Zero;
    }

    private bool IsCurrentCursorIBeam()
    {
        var ci = new NativeMethods.CURSORINFO { cbSize = Marshal.SizeOf<NativeMethods.CURSORINFO>() };
        return NativeMethods.GetCursorInfo(ref ci)
            && (ci.flags & NativeMethods.CURSOR_SHOWING) != 0
            && ci.hCursor == _hIBeamCursor;
    }

    private bool IsPointOnOwnProcess(NativeMethods.POINT pt)
    {
        IntPtr hwnd = NativeMethods.WindowFromPoint(pt);
        if (hwnd == IntPtr.Zero) return false;
        NativeMethods.GetWindowThreadProcessId(hwnd, out uint pid);
        return pid == _currentPid;
    }

    private IntPtr MouseHookCallback(int nCode, IntPtr wParam, IntPtr lParam)
    {
        if (nCode >= 0)
        {
            int msg = wParam.ToInt32();
            if (msg is NativeMethods.WM_LBUTTONDOWN or NativeMethods.WM_LBUTTONUP or NativeMethods.WM_RBUTTONDOWN or NativeMethods.WM_MOUSEWHEEL)
            {
                var hookStruct = Marshal.PtrToStructure<NativeMethods.MSLLHOOKSTRUCT>(lParam);
                if ((hookStruct.flags & NativeMethods.LLMHF_INJECTED) == 0)
                {
                    var pt = hookStruct.pt;
                    long now = Environment.TickCount64;

                    if (msg == NativeMethods.WM_LBUTTONDOWN)
                    {
                        if (!IsPointOnOwnProcess(pt))
                        {
                            _isLeftButtonDown = true;
                            _mouseDownPt = pt;
                            _mouseDownTick = now;
                            _mouseDownWasIBeam = EnableFloatingBadge && IsCurrentCursorIBeam();
                            System.Windows.Application.Current?.Dispatcher.BeginInvoke(() => ExternalMouseAction?.Invoke());
                        }
                    }
                    else if (msg is NativeMethods.WM_RBUTTONDOWN or NativeMethods.WM_MOUSEWHEEL)
                    {
                        if (!IsPointOnOwnProcess(pt))
                        {
                            System.Windows.Application.Current?.Dispatcher.BeginInvoke(() => ExternalMouseAction?.Invoke());
                        }
                    }
                    else if (msg == NativeMethods.WM_LBUTTONUP && _isLeftButtonDown)
                    {
                        _isLeftButtonDown = false;

                        if (EnableFloatingBadge && !IsPointOnOwnProcess(pt) && (_mouseDownWasIBeam || IsCurrentCursorIBeam()))
                        {
                            int dx = pt.X - _mouseDownPt.X;
                            int dy = pt.Y - _mouseDownPt.Y;
                            double dist = Math.Sqrt(dx * dx + dy * dy);
                            long dragDuration = now - _mouseDownTick;

                            bool isDragSelect = dist >= 14 && dragDuration is >= 40 and <= 15000;

                            int dblDx = pt.X - _lastClickUpPt.X;
                            int dblDy = pt.Y - _lastClickUpPt.Y;
                            bool isDoubleClickWord = dist < 8
                                && Math.Sqrt(dblDx * dblDx + dblDy * dblDy) < 8
                                && (now - _lastClickUpTick) is >= 40 and <= 400;

                            _lastClickUpPt = pt;
                            _lastClickUpTick = now;

                            if (isDragSelect || isDoubleClickWord)
                            {
                                System.Windows.Application.Current?.Dispatcher.BeginInvoke(
                                    () => TextSelectionSuspected?.Invoke(pt));
                            }
                        }
                    }
                }
            }
        }

        return NativeMethods.CallNextHookEx(_mouseHookId, nCode, wParam, lParam);
    }

    private IntPtr KeyboardHookCallback(int nCode, IntPtr wParam, IntPtr lParam)
    {
        if (nCode >= 0)
        {
            int msg = wParam.ToInt32();
            if (msg is NativeMethods.WM_KEYDOWN or NativeMethods.WM_SYSKEYDOWN)
            {
                var kb = Marshal.PtrToStructure<NativeMethods.KBDLLHOOKSTRUCT>(lParam);

                if ((kb.flags & NativeMethods.LLKHF_INJECTED) == 0)
                {
                    if (kb.vkCode == NativeMethods.VK_ESCAPE)
                    {
                        System.Windows.Application.Current?.Dispatcher.BeginInvoke(() => EscapePressed?.Invoke());
                    }
                    else if (EnableDoubleCtrlC && kb.vkCode == NativeMethods.VK_C)
                    {
                        bool ctrlDown = (NativeMethods.GetAsyncKeyState(NativeMethods.VK_CONTROL) & 0x8000) != 0;
                        bool altDown = (NativeMethods.GetAsyncKeyState(NativeMethods.VK_MENU) & 0x8000) != 0;
                        bool shiftDown = (NativeMethods.GetAsyncKeyState(NativeMethods.VK_SHIFT) & 0x8000) != 0;

                        if (ctrlDown && !altDown && !shiftDown)
                        {
                            long now = Environment.TickCount64;
                            if (now - _lastCtrlCTick is >= 60 and <= 450)
                            {
                                _lastCtrlCTick = 0;
                                System.Windows.Application.Current?.Dispatcher.BeginInvoke(() => DoubleCopyTriggered?.Invoke());
                            }
                            else
                            {
                                _lastCtrlCTick = now;
                            }
                        }
                    }
                    else if (kb.vkCode != NativeMethods.VK_CONTROL)
                    {
                        _lastCtrlCTick = 0;
                    }
                }
            }
        }

        return NativeMethods.CallNextHookEx(_keyboardHookId, nCode, wParam, lParam);
    }

    public void Dispose()
    {
        if (_mouseHookId != IntPtr.Zero)
        {
            NativeMethods.UnhookWindowsHookEx(_mouseHookId);
            _mouseHookId = IntPtr.Zero;
        }
        if (_keyboardHookId != IntPtr.Zero)
        {
            NativeMethods.UnhookWindowsHookEx(_keyboardHookId);
            _keyboardHookId = IntPtr.Zero;
        }
        if (_hwndSource != null)
        {
            NativeMethods.UnregisterHotKey(_hwndSource.Handle, HOTKEY_TRANSLATE_ID);
            _hwndSource.Dispose();
            _hwndSource = null;
        }
    }
}
