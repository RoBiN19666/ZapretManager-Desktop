using System;
using System.IO;
using System.Text;

namespace ZapretManager
{
    /// <summary>
    /// Автозапуск менеджера при входе в Windows через Планировщик заданий
    /// (задача с наивысшими правами — запуск без запроса UAC).
    /// </summary>
    public static class Autostart
    {
        public const string TaskName = "ZapretManager";
        public const string AutostartArg = "--autostart";

        public static bool IsEnabled
        {
            get { return Query().Length > 0; }
        }

        /// <summary>Возвращает описание задачи или пустую строку, если её нет.</summary>
        public static string Query()
        {
            ProcResult r = Proc.Run("schtasks.exe", string.Format("/Query /TN \"{0}\" /FO LIST", TaskName), 15000);
            if (r.ExitCode != 0) return "";
            return r.All.Trim();
        }

        /// <summary>Создаёт задачу автозапуска (нужны права администратора).</summary>
        public static string Enable(string exePath, bool startMinimized)
        {
            if (!Native.IsAdmin()) return "Для автозапуска нужны права администратора.";
            if (!File.Exists(exePath)) return "Файл программы не найден: " + exePath;

            StringBuilder args = new StringBuilder();
            args.Append("/Create /TN \"").Append(TaskName).Append("\"");
            args.Append(" /TR \"\\\"").Append(exePath).Append("\\\" ").Append(AutostartArg);
            if (startMinimized) args.Append(" --tray");
            args.Append("\"");
            args.Append(" /SC ONLOGON /RL HIGHEST /F");
            ProcResult r = Proc.Run("schtasks.exe", args.ToString(), 30000);
            if (r.ExitCode != 0)
            {
                Log.Warn("schtasks create: " + r.All);
                return "Не удалось создать задачу автозапуска: " + Shorten(r.All);
            }
            Log.Ok("Автозапуск включён (задача " + TaskName + ").");
            return startMinimized
                ? "Автозапуск включён: менеджер будет стартовать свёрнутым в трей."
                : "Автозапуск включён: менеджер будет запускаться при входе в Windows.";
        }

        public static string Disable()
        {
            if (!Native.IsAdmin()) return "Для изменения автозапуска нужны права администратора.";
            ProcResult r = Proc.Run("schtasks.exe", string.Format("/Delete /TN \"{0}\" /F", TaskName), 20000);
            if (r.ExitCode != 0 && Query().Length > 0)
            {
                Log.Warn("schtasks delete: " + r.All);
                return "Не удалось удалить задачу автозапуска: " + Shorten(r.All);
            }
            Log.Info("Автозапуск выключен.");
            return "Автозапуск выключен.";
        }

        /// <summary>Открывает Планировщик заданий на нашей задаче.</summary>
        public static void OpenTaskScheduler()
        {
            try { Proc.StartHidden("taskschd.msc", "", ""); }
            catch { }
        }

        private static string Shorten(string s)
        {
            if (string.IsNullOrEmpty(s)) return "";
            s = s.Replace("\r", " ").Replace("\n", " ").Trim();
            return s.Length > 200 ? s.Substring(0, 200) + "…" : s;
        }
    }
}
