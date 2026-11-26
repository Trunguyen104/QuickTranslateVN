using System.Windows;
using System.Windows.Input;
using System.Windows.Threading;
using QuickTranslateVN.Interop;

namespace QuickTranslateVN.UI;

public partial class FloatingBadgeWindow : Window
{
    private readonly DispatcherTimer _autoHideTimer;
    public NativeMethods.POINT TriggerPoint { get; private set; }

    public event Action<NativeMethods.POINT>? BadgeClicked;

    public FloatingBadgeWindow()
    {
        InitializeComponent();
        WindowPositionHelper.MakeNonActivating(this);

        _autoHideTimer = new DispatcherTimer
        {
            Interval = TimeSpan.FromSeconds(4.0)
        };
        _autoHideTimer.Tick += (_, _) => HideBadge();
    }

    public void ShowAt(NativeMethods.POINT screenPt)
    {
        TriggerPoint = screenPt;
        _autoHideTimer.Stop();
        WindowPositionHelper.ShowAndPositionNearPhysicalPoint(this, screenPt, offsetDipX: 10, offsetDipY: 12);
        _autoHideTimer.Start();
    }

    public void HideBadge()
    {
        _autoHideTimer.Stop();
        if (IsVisible)
            Hide();
    }

    private void BadgeBorder_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
    {
        e.Handled = true;
        var pt = TriggerPoint;
        HideBadge();
        BadgeClicked?.Invoke(pt);
    }
}
