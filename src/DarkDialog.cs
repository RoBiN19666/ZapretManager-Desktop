using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
using System.Windows.Forms;

namespace ZapretManager
{
    /// <summary>
    /// Красит системные диалоги (выбор файла/папки, message box) в тёмный режим:
    /// ставит им тёмный заголовок и тему DarkMode_Explorer на все дочерние контролы.
    /// </summary>
    public sealed class DarkDialogWatcher : IDisposable
    {
        [DllImport("uxtheme.dll", CharSet = CharSet.Unicode)]
        private static extern int SetWindowTheme(IntPtr hWnd, string pszSubAppName, string pszSubIdList);

        [DllImport("dwmapi.dll")]
        private static extern int DwmSetWindowAttribute(IntPtr hwnd, int attr, ref int attrValue, int attrSize);

        [DllImport("user32.dll")]
        private static extern bool EnumWindows(EnumWindowsProc proc, IntPtr lParam);

        [DllImport("user32.dll")]
        private static extern bool EnumChildWindows(IntPtr parent, EnumWindowsProc proc, IntPtr lParam);

        [DllImport("user32.dll")]
        private static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);

        [DllImport("user32.dll", CharSet = CharSet.Unicode)]
        private static extern int GetClassName(IntPtr hwnd, StringBuilder name, int count);

        [DllImport("user32.dll")]
        private static extern bool IsWindowVisible(IntPtr hwnd);

        private delegate bool EnumWindowsProc(IntPtr hwnd, IntPtr lParam);

        private readonly Timer _timer;
        private readonly HashSet<IntPtr> _painted = new HashSet<IntPtr>();

        public DarkDialogWatcher()
        {
            _timer = new Timer();
            _timer.Interval = 120;
            _timer.Tick += delegate { Sweep(); };
            _timer.Start();
        }

        public void Dispose()
        {
            try { _timer.Stop(); _timer.Dispose(); } catch { }
        }

        private void Sweep()
        {
            if (!Theme.IsDark) return;
            if (_painted.Count > 64) _painted.Clear();   // защита от накопления/переиспользования HWND
            int dark = 1;
            uint pid = (uint)System.Diagnostics.Process.GetCurrentProcess().Id;
            EnumWindows(delegate(IntPtr hwnd, IntPtr lParam)
            {
                uint winPid;
                GetWindowThreadProcessId(hwnd, out winPid);
                if (winPid != pid || !IsWindowVisible(hwnd)) return true;
                if (_painted.Contains(hwnd)) return true;
                StringBuilder cls = new StringBuilder(64);
                GetClassName(hwnd, cls, cls.Capacity);
                string name = cls.ToString();
                // только диалоги: выбор файла/папки, message box
                if (name != "#32770") return true;
                _painted.Add(hwnd);
                try { DwmSetWindowAttribute(hwnd, 20, ref dark, 4); } catch { }
                try { SetWindowTheme(hwnd, "DarkMode_Explorer", null); } catch { }
                EnumChildWindows(hwnd, delegate(IntPtr child, IntPtr l)
                {
                    try { SetWindowTheme(child, "DarkMode_Explorer", null); } catch { }
                    return true;
                }, IntPtr.Zero);
                return true;
            }, IntPtr.Zero);
        }
    }
}
