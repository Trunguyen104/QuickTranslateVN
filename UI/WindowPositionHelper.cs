using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Interop;
using QuickTranslateVN.Interop;

namespace QuickTranslateVN.UI;

internal static class WindowPositionHelper
{
    private static readonly HashSet<IntPtr> InteractiveHwnds = [];

    public static void MakeNonActivating(Window window)
    {
        window.ShowActivated = false;
        window.SourceInitialized += (_, _) =>
        {
            var helper = new WindowInteropHelper(window);
            IntPtr hwnd = helper.Handle;

            long exStyle = NativeMethods.GetWindowLongPtr(hwnd, NativeMethods.GWL_EXSTYLE).ToInt64();
            exStyle |= NativeMethods.WS_EX_NOACTIVATE | NativeMethods.WS_EX_TOOLWINDOW | NativeMethods.WS_EX_TOPMOST;
            NativeMethods.SetWindowLongPtr(hwnd, NativeMethods.GWL_EXSTYLE, new IntPtr(exStyle));

            var source = HwndSource.FromHwnd(hwnd);
            source?.AddHook(NonActivatingWndProc);
        };
    }

    /// <summary>
    /// Temporarily toggles keyboard focusability when user opens Quick Input Mode (Ctrl+Shift+T)
    /// vs non-activating selection mode (Alt+Q / Mouse / Ctrl+C+C).
    /// </summary>
    public static void SetInteractiveFocusMode(Window window, bool interactive)
    {
        var hwnd = new WindowInteropHelper(window).Handle;
        if (hwnd == IntPtr.Zero) return;

        long exStyle = NativeMethods.GetWindowLongPtr(hwnd, NativeMethods.GWL_EXSTYLE).ToInt64();
        if (interactive)
        {
            InteractiveHwnds.Add(hwnd);
            exStyle &= ~NativeMethods.WS_EX_NOACTIVATE;
            NativeMethods.SetWindowLongPtr(hwnd, NativeMethods.GWL_EXSTYLE, new IntPtr(exStyle));
            NativeMethods.SetForegroundWindow(hwnd);
            window.Activate();
        }
        else
        {
            InteractiveHwnds.Remove(hwnd);
            exStyle |= NativeMethods.WS_EX_NOACTIVATE | NativeMethods.WS_EX_TOOLWINDOW | NativeMethods.WS_EX_TOPMOST;
            NativeMethods.SetWindowLongPtr(hwnd, NativeMethods.GWL_EXSTYLE, new IntPtr(exStyle));
        }
    }

    private static IntPtr NonActivatingWndProc(IntPtr hwnd, int msg, IntPtr wParam, IntPtr lParam, ref bool handled)
    {
        if (msg == NativeMethods.WM_MOUSEACTIVATE && !InteractiveHwnds.Contains(hwnd))
        {
            handled = true;
            return new IntPtr(NativeMethods.MA_NOACTIVATE);
        }
        return IntPtr.Zero;
    }

    public static void ShowAndPositionNearPhysicalPoint(
        Window window,
        NativeMethods.POINT anchorPhysical,
        double offsetDipX = 10,
        double offsetDipY = 14)
    {
        IntPtr hMonitor = NativeMethods.MonitorFromPoint(anchorPhysical, NativeMethods.MONITOR_DEFAULTTONEAREST);

        double scaleX = 1.0;
        double scaleY = 1.0;
        if (hMonitor != IntPtr.Zero && NativeMethods.GetDpiForMonitor(hMonitor, 0, out uint dpiX, out uint dpiY) == 0 && dpiX > 0)
        {
            scaleX = dpiX / 96.0;
            scaleY = dpiY / 96.0;
        }

        var mi = new NativeMethods.MONITORINFO { cbSize = Marshal.SizeOf<NativeMethods.MONITORINFO>() };
        double workLeft = SystemParameters.WorkArea.Left;
        double workTop = SystemParameters.WorkArea.Top;
        double workRight = SystemParameters.WorkArea.Right;
        double workBottom = SystemParameters.WorkArea.Bottom;

        if (hMonitor != IntPtr.Zero && NativeMethods.GetMonitorInfo(hMonitor, ref mi))
        {
            workLeft = mi.rcWork.Left / scaleX;
            workTop = mi.rcWork.Top / scaleY;
            workRight = mi.rcWork.Right / scaleX;
            workBottom = mi.rcWork.Bottom / scaleY;
        }

        double dipX = (anchorPhysical.X / scaleX) + offsetDipX;
        double dipY = (anchorPhysical.Y / scaleY) + offsetDipY;

        double winW = window.ActualWidth > 10 ? window.ActualWidth : (double.IsNaN(window.Width) ? 430 : window.Width);
        double winH = window.ActualHeight > 10 ? window.ActualHeight : (double.IsNaN(window.Height) ? 220 : window.Height);

        const double margin = 10;

        if (dipX + winW > workRight - margin)
            dipX = Math.Max(workLeft + margin, workRight - winW - margin);

        if (dipX < workLeft + margin)
            dipX = workLeft + margin;

        if (dipY + winH > workBottom - margin)
        {
            double flippedY = (anchorPhysical.Y / scaleY) - winH - 18;
            dipY = flippedY >= workTop + margin ? flippedY : Math.Max(workTop + margin, workBottom - winH - margin);
        }

        window.Left = dipX;
        window.Top = dipY;

        if (!window.IsVisible)
        {
            window.Show();
        }

        var hwnd = new WindowInteropHelper(window).Handle;
        if (hwnd != IntPtr.Zero)
        {
            NativeMethods.SetWindowPos(
                hwnd,
                NativeMethods.HWND_TOPMOST,
                0, 0, 0, 0,
                NativeMethods.SWP_NOSIZE | NativeMethods.SWP_NOACTIVATE | NativeMethods.SWP_SHOWWINDOW);
            window.Left = dipX;
            window.Top = dipY;
        }
    }
}
