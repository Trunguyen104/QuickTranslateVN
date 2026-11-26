using System.Drawing;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using QuickTranslateVN.Interop;

namespace QuickTranslateVN.UI;

public partial class OcrOverlayWindow : Window
{
    private bool _isSelecting;
    private System.Windows.Point _startDip;
    private NativeMethods.POINT _startPhysical;

    public Rectangle? SelectedPhysicalRect { get; private set; }
    public NativeMethods.POINT EndCursorPoint { get; private set; }

    public OcrOverlayWindow()
    {
        InitializeComponent();

        Left = SystemParameters.VirtualScreenLeft;
        Top = SystemParameters.VirtualScreenTop;
        Width = SystemParameters.VirtualScreenWidth;
        Height = SystemParameters.VirtualScreenHeight;

        MouseRightButtonDown += (_, _) =>
        {
            SelectedPhysicalRect = null;
            DialogResult = false;
            Close();
        };
    }

    private void Window_KeyDown(object sender, System.Windows.Input.KeyEventArgs e)
    {
        if (e.Key == Key.Escape)
        {
            SelectedPhysicalRect = null;
            DialogResult = false;
            Close();
        }
    }

    private void Canvas_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
    {
        _isSelecting = true;
        _startDip = e.GetPosition(OverlayCanvas);
        NativeMethods.GetCursorPos(out _startPhysical);

        Canvas.SetLeft(SelectionRect, _startDip.X);
        Canvas.SetTop(SelectionRect, _startDip.Y);
        SelectionRect.Width = 0;
        SelectionRect.Height = 0;
        SelectionRect.Visibility = Visibility.Visible;
        OverlayCanvas.CaptureMouse();
    }

    private void Canvas_MouseMove(object sender, System.Windows.Input.MouseEventArgs e)
    {
        if (!_isSelecting) return;
        var cur = e.GetPosition(OverlayCanvas);

        double x = Math.Min(_startDip.X, cur.X);
        double y = Math.Min(_startDip.Y, cur.Y);
        double w = Math.Abs(cur.X - _startDip.X);
        double h = Math.Abs(cur.Y - _startDip.Y);

        Canvas.SetLeft(SelectionRect, x);
        Canvas.SetTop(SelectionRect, y);
        SelectionRect.Width = w;
        SelectionRect.Height = h;
    }

    private void Canvas_MouseLeftButtonUp(object sender, MouseButtonEventArgs e)
    {
        if (!_isSelecting) return;
        _isSelecting = false;
        OverlayCanvas.ReleaseMouseCapture();

        NativeMethods.GetCursorPos(out var endPhysical);
        EndCursorPoint = endPhysical;

        int left = Math.Min(_startPhysical.X, endPhysical.X);
        int top = Math.Min(_startPhysical.Y, endPhysical.Y);
        int width = Math.Abs(endPhysical.X - _startPhysical.X);
        int height = Math.Abs(endPhysical.Y - _startPhysical.Y);

        if (width >= 10 && height >= 10)
        {
            SelectedPhysicalRect = new Rectangle(left, top, width, height);
            DialogResult = true;
        }
        else
        {
            SelectedPhysicalRect = null;
            DialogResult = false;
        }

        Close();
    }
}
