using System;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
using System.Windows.Forms;

namespace ZapretManager
{
    /// <summary>Проверка системной тёмной темы: показывает нативное окно и снимает его.</summary>
    public static class DarkModeTest
    {
        [DllImport("user32.dll", CharSet = CharSet.Unicode)]
        private static extern IntPtr FindWindow(string cls, string title);

        [DllImport("user32.dll")]
        private static extern bool GetWindowRect(IntPtr hwnd, out RECT rect);

        [DllImport("user32.dll")]
        private static extern bool SetProcessDPIAware();

        [DllImport("user32.dll")]
        private static extern bool PostMessage(IntPtr hwnd, int msg, IntPtr w, IntPtr l);

        [StructLayout(LayoutKind.Sequential)]
        private struct RECT { public int Left, Top, Right, Bottom; }

        public static void Run(string path)
        {
            SetProcessDPIAware();
            Timer timer = new Timer();
            timer.Interval = 1500;
            timer.Tick += delegate(object s, EventArgs e)
            {
                timer.Stop();
                try
                {
                    IntPtr hwnd = FindWindow("#32770", "Проверка тёмной темы");
                    if (hwnd == IntPtr.Zero) hwnd = FindWindow("#32770", null);
                    if (hwnd != IntPtr.Zero)
                    {
                        RECT r;
                        if (GetWindowRect(hwnd, out r))
                        {
                            int w = Math.Max(10, r.Right - r.Left);
                            int h = Math.Max(10, r.Bottom - r.Top);
                            using (Bitmap bmp = new Bitmap(w, h))
                            {
                                using (Graphics g = Graphics.FromImage(bmp)) g.CopyFromScreen(r.Left, r.Top, 0, 0, new Size(w, h));
                                bmp.Save(path, ImageFormat.Png);
                            }
                            Log.Info("Снимок окна проверки темы: " + path);
                        }
                        PostMessage(hwnd, 0x0010, IntPtr.Zero, IntPtr.Zero);   // WM_CLOSE
                    }
                    else
                    {
                        Log.Warn("Окно проверки темы не найдено.");
                    }
                }
                catch (Exception ex)
                {
                    Log.Warn("Проверка темы: " + ex.Message);
                }
                finally
                {
                    Application.ExitThread();
                }
            };
            timer.Start();
            using (DarkDialogWatcher watcher = new DarkDialogWatcher())
            {
                MessageBox.Show("Если это окно тёмное — системные диалоги тоже будут тёмными.", "Проверка тёмной темы",
                    MessageBoxButtons.OK, MessageBoxIcon.Information);
            }
        }
    }
}
