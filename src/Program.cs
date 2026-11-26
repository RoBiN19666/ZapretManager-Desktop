using System;
using System.Diagnostics;
using System.Drawing;
using System.IO;
using System.Text;
using System.Threading;
using System.Windows.Forms;

namespace ZapretManager
{
    internal static class Program
    {
        private static Mutex _mutex;

        [STAThread]
        private static void Main(string[] args)
        {
            Web.EnableModernTls();
            Application.EnableVisualStyles();
            Application.SetCompatibleTextRenderingDefault(false);
            Application.SetUnhandledExceptionMode(UnhandledExceptionMode.CatchException);
            Application.ThreadException += delegate(object s, ThreadExceptionEventArgs e) { ReportCrash(e.Exception); };
            AppDomain.CurrentDomain.UnhandledException += delegate(object s, UnhandledExceptionEventArgs e) { ReportCrash(e.ExceptionObject as Exception); };

            string rootArg = FindArgValue(args, "--root");
            AppInfo.ShotPath = FindArgValue(args, "--shot");
            AppInfo.ShotPage = FindArgValue(args, "--shot-page");
            AppInfo.ShotList = FindArgValue(args, "--shot-list");
            int shotScroll;
            if (int.TryParse(FindArgValue(args, "--shot-scroll"), out shotScroll)) AppInfo.ShotScroll = shotScroll;
            bool shotMode = !string.IsNullOrEmpty(AppInfo.ShotPath);
            bool selfUpdateMode = HasFlag(args, "--self-update");
            AppInfo.TrayMode = HasFlag(args, "--tray");
            AppInfo.AutostartMode = HasFlag(args, "--autostart");
            if (AppInfo.AutostartMode) AppInfo.TrayMode = true;
            bool limited = HasFlag(args, "--limited") || shotMode;
            bool elevated = HasFlag(args, "--elevated");

            if (!Native.IsAdmin() && !limited && !elevated && !shotMode)
            {
                // Пытаемся сразу перезапуститься с правами администратора: управление службами и winws.exe требуют elevation.
                try
                {
                    ProcessStartInfo psi = new ProcessStartInfo(Application.ExecutablePath, BuildElevatedArgs(args));
                    psi.UseShellExecute = true;
                    psi.Verb = "runas";
                    psi.WorkingDirectory = Path.GetDirectoryName(Application.ExecutablePath);
                    Process.Start(psi);
                    return;
                }
                catch
                {
                    // Пользователь отказался от UAC — работаем в ограниченном режиме (просмотр и настройка списков).
                }
            }

            try { _mutex = new Mutex(true, "ZapretManager_SingleInstance", out _mutexCreated); }
            catch { _mutexCreated = true; }
            if (!_mutexCreated && !shotMode)
            {
                ActivateExistingWindow();
                return;
            }
            try
            {
                AppInfo.ActivateEvent = new EventWaitHandle(false, EventResetMode.AutoReset, "ZapretManager_Activate");
            }
            catch { }
            Log.FilePath = Path.Combine(AppPaths.DataDir, "manager.log");

            Log.Info("=== Zapret Manager " + AppInfo.Version + " запуск ===");
            if (selfUpdateMode)
            {
                RunSelfUpdateMode();
                return;
            }
            Log.Info("Аргументы: [" + string.Join("] [", args) + "]");
            Log.Info("Права администратора: " + (Native.IsAdmin() ? "да" : "нет") + " / " + (limited ? "ограниченный режим" : "обычный режим"));

            AppConfig config = AppConfig.Load(AppConfig.DefaultPath());
            string preferred = rootArg;
            if (string.IsNullOrEmpty(preferred)) preferred = config.Get("Root", "");
            ZapretInstall install = ZapretInstall.Detect(preferred);
            Log.Info("Папка zapret: " + install.Root + (install.IsValid ? " (версия " + install.Version + ")" : " (не найдена)"));

            Shell shell = new Shell();
            shell.Config = config;
            shell.Install = install;
            shell.Ops = new ZapretOps(install);
            if (install.IsValid && config.Get("Root", "") != install.Root)
            {
                config.Set("Root", install.Root);
                config.Save();
            }

            MainForm form = new MainForm(shell);
            int w = config.GetInt("WindowW", 1280);
            int h = config.GetInt("WindowH", 880);
            int x = config.GetInt("WindowX", int.MinValue);
            int y = config.GetInt("WindowY", int.MinValue);
            form.Width = Math.Max(form.MinimumSize.Width, w);
            form.Height = Math.Max(form.MinimumSize.Height, h);
            if (x != int.MinValue && y != int.MinValue)
            {
                Rectangle wa = Screen.PrimaryScreen.WorkingArea;
                if (x > wa.Left - 200 && y > wa.Top - 200 && x < wa.Right - 100 && y < wa.Bottom - 100)
                {
                    form.StartPosition = FormStartPosition.Manual;
                    form.Location = new Point(x, y);
                }
            }
            if (config.GetBool("Maximized", false)) form.WindowState = FormWindowState.Maximized;

            try
            {
                Application.Run(form);
            }
            finally
            {
                Log.Info("=== Завершение работы ===");
                if (_mutex != null) { try { _mutex.ReleaseMutex(); } catch { } }
            }
        }

        private static bool _mutexCreated;

        private static void ActivateExistingWindow()
        {
            // просим уже запущенный экземпляр показать окно
            try
            {
                using (EventWaitHandle ev = EventWaitHandle.OpenExisting("ZapretManager_Activate"))
                {
                    ev.Set();
                    Log.Info("Передал запрос на показ окна уже запущенному экземпляру.");
                }
            }
            catch { }
            for (int i = 0; i < 6; i++)
            {
                IntPtr h = Native.FindWindow(null, AppInfo.Title);
                if (h != IntPtr.Zero)
                {
                    Native.ShowWindow(h, 9);
                    Native.SetForegroundWindow(h);
                    return;
                }
                Thread.Sleep(300);
            }
        }

        private static void ReportCrash(Exception ex)
        {
            try
            {
                Log.Write(LogLevel.Error, "Необработанная ошибка: " + (ex == null ? "unknown" : ex.ToString()));
                MessageBox.Show("Произошла ошибка:\n\n" + (ex == null ? "неизвестная" : ex.Message) +
                    "\n\nПодробности записаны в журнал:\n" + Path.Combine(AppPaths.DataDir, "manager.log"),
                    AppInfo.Title, MessageBoxButtons.OK, MessageBoxIcon.Error);
            }
            catch { }
        }

        /// <summary>Служебный режим самообновления без интерфейса (--self-update).</summary>
        private static void RunSelfUpdateMode()
        {
            try
            {
                SelfUpdateInfo info = SelfUpdate.Check(true);
                Log.Info(string.Format("Самообновление: локальная {0}, в репозитории {1}, доступно={2}{3}",
                    info.Local, info.Remote, info.Available, info.Ok ? "" : " (" + info.Error + ")"));
                if (!info.Ok || !info.Available)
                {
                    Log.Info("Обновление не требуется.");
                    return;
                }
                bool ok;
                string pending;
                string msg = SelfUpdate.Apply(info, null, out ok, out pending);
                Log.Write(ok ? LogLevel.Ok : LogLevel.Error, "Самообновление: " + msg);
            }
            catch (Exception ex)
            {
                Log.Error("Самообновление: ошибка", ex);
            }
        }

        private static bool HasFlag(string[] args, string flag)
        {
            foreach (string a in args)
            {
                if (string.Equals(a, flag, StringComparison.OrdinalIgnoreCase)) return true;
            }
            return false;
        }

        private static string FindArgValue(string[] args, string name)
        {
            for (int i = 0; i < args.Length; i++)
            {
                if (string.Equals(args[i], name, StringComparison.OrdinalIgnoreCase))
                {
                    return i + 1 < args.Length ? args[i + 1] : null;
                }
                if (args[i].StartsWith(name + "=", StringComparison.OrdinalIgnoreCase))
                {
                    return args[i].Substring(name.Length + 1).Trim('"');
                }
            }
            return null;
        }

        /// <summary>Собирает аргументы для перезапуска с правами администратора.</summary>
        private static string BuildElevatedArgs(string[] args)
        {
            StringBuilder sb = new StringBuilder();
            sb.Append("--elevated");
            for (int i = 0; i < args.Length; i++)
            {
                string a = args[i];
                if (string.Equals(a, "--elevated", StringComparison.OrdinalIgnoreCase)) continue;
                if (string.Equals(a, "--limited", StringComparison.OrdinalIgnoreCase)) continue;
                sb.Append(' ');
                if (a.IndexOf(' ') >= 0) sb.Append('"').Append(a).Append('"');
                else sb.Append(a);
            }
            return sb.ToString();
        }
    }

    public static class AppIcon
    {
        public static Icon Get()
        {
            try { return Icon.ExtractAssociatedIcon(Application.ExecutablePath); }
            catch { return SystemIcons.Application; }
        }
    }
}
