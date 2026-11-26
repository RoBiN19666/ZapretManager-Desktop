using System;
using System.Collections.Generic;
using System.IO;
using System.Text;

namespace ZapretManager
{
    /// <summary>
    /// Работа с официальными списками zapret: пользовательские добавления и удаления
    /// помечаются, чтобы при обновлении zapret они не потерялись, а новые строки
    /// разработчика при этом доехали до пользователя.
    /// </summary>
    public sealed class ListChanges
    {
        public const string BlockStart = "# >>> Zapret Manager: мои добавления (сохраняются при обновлении zapret)";
        public const string BlockEnd = "# <<< Zapret Manager: конец моих добавлений";

        public string FileName = "";
        public string FilePath = "";
        public string DataDir = "";

        public List<string> Added = new List<string>();
        public List<string> Removed = new List<string>();
        public List<string> BaselineEntries = new List<string>();
        public List<string> CurrentEntries = new List<string>();
        public List<string> News = new List<string>();
        public bool HasBaseline;
        public string LastError = "";

        /// <summary>Официальные списки, которые обновляются вместе с zapret.</summary>
        public static readonly string[] OfficialFiles = new string[]
        {
            "list-general.txt", "list-google.txt", "list-exclude.txt", "ipset-exclude.txt"
        };

        public static bool IsOfficial(string fileName)
        {
            if (string.IsNullOrEmpty(fileName)) return false;
            foreach (string f in OfficialFiles)
            {
                if (string.Equals(f, fileName, StringComparison.OrdinalIgnoreCase)) return true;
            }
            return false;
        }

        private static string ChangesDir(string dataDir)
        {
            string d = Path.Combine(dataDir, "lists");
            if (!Directory.Exists(d)) Directory.CreateDirectory(d);
            return d;
        }

        public static string ChangesPath(string dataDir, string fileName)
        {
            return Path.Combine(ChangesDir(dataDir), fileName + ".changes");
        }

        public static string BaselinePath(string dataDir, string fileName)
        {
            return Path.Combine(ChangesDir(dataDir), fileName + ".baseline");
        }

        public static string NewsPath(string dataDir, string fileName)
        {
            return Path.Combine(ChangesDir(dataDir), fileName + ".news");
        }

        // ---------- разбор текста ----------

        /// <summary>Значимые строки: без пустых и комментариев.</summary>
        public static List<string> ParseEntries(IEnumerable<string> lines)
        {
            List<string> list = new List<string>();
            foreach (string raw in lines)
            {
                string line = raw.Trim().TrimStart('\uFEFF');
                if (line.Length == 0) continue;
                if (line.StartsWith("#")) continue;
                list.Add(line);
            }
            return list;
        }

        /// <summary>Отделяет «официальную» часть файла от нашего блока добавлений.</summary>
        public static void SplitBlock(string[] lines, out List<string> officialLines, out List<string> blockEntries)
        {
            officialLines = new List<string>();
            blockEntries = new List<string>();
            bool inBlock = false;
            foreach (string raw in lines)
            {
                string line = raw.TrimEnd('\r', '\n');
                string trimmed = line.Trim();
                if (!inBlock && trimmed.StartsWith("# >>> Zapret Manager", StringComparison.OrdinalIgnoreCase))
                {
                    inBlock = true;
                    continue;
                }
                if (inBlock)
                {
                    if (trimmed.StartsWith("# <<< Zapret Manager", StringComparison.OrdinalIgnoreCase))
                    {
                        inBlock = false;
                        continue;
                    }
                    if (trimmed.Length > 0 && !trimmed.StartsWith("#")) blockEntries.Add(trimmed);
                    continue;
                }
                officialLines.Add(line);
            }
        }

        // ---------- загрузка / сохранение ----------

        public static ListChanges Load(string filePath, string dataDir)
        {
            ListChanges lc = new ListChanges();
            lc.FilePath = filePath;
            lc.FileName = Path.GetFileName(filePath);
            lc.DataDir = dataDir;
            try
            {
                string[] lines = File.Exists(filePath) ? File.ReadAllLines(filePath, Encoding.UTF8) : new string[0];
                List<string> officialLines;
                List<string> blockEntries;
                SplitBlock(lines, out officialLines, out blockEntries);
                lc.CurrentEntries = ParseEntries(officialLines);
                lc.Added = new List<string>(blockEntries);

                string basePath = BaselinePath(dataDir, lc.FileName);
                if (File.Exists(basePath))
                {
                    lc.BaselineEntries = ParseEntries(File.ReadAllLines(basePath, Encoding.UTF8));
                    lc.HasBaseline = true;
                }

                string chPath = ChangesPath(dataDir, lc.FileName);
                if (File.Exists(chPath))
                {
                    foreach (string raw in File.ReadAllLines(chPath, Encoding.UTF8))
                    {
                        string line = raw.Trim();
                        if (line.Length < 2) continue;
                        if (line[0] == '+') AddUnique(lc.Added, line.Substring(1).Trim());
                        else if (line[0] == '-') AddUnique(lc.Removed, line.Substring(1).Trim());
                    }
                }

                // добавления, набранные прямо в файле, тоже считаем своими
                if (lc.HasBaseline)
                {
                    foreach (string e in lc.CurrentEntries)
                    {
                        if (!Contains(lc.BaselineEntries, e)) AddUnique(lc.Added, e);
                    }
                    foreach (string e in lc.BaselineEntries)
                    {
                        if (!Contains(lc.CurrentEntries, e) && !Contains(lc.Added, e)) AddUnique(lc.Removed, e);
                    }
                }
                else
                {
                    // базовой версии ещё нет — фиксируем текущее содержимое как официальное
                    lc.BaselineEntries = new List<string>(lc.CurrentEntries);
                    lc.HasBaseline = true;
                    lc.SaveBaseline();
                }

                string newsPath = NewsPath(dataDir, lc.FileName);
                if (File.Exists(newsPath)) lc.News = ParseEntries(File.ReadAllLines(newsPath, Encoding.UTF8));

                // наши «добавления» не должны дублировать официальные строки
                List<string> cleanAdded = new List<string>();
                foreach (string a in lc.Added)
                {
                    if (!Contains(lc.BaselineEntries, a) && !Contains(cleanAdded, a)) cleanAdded.Add(a);
                }
                lc.Added = cleanAdded;
                List<string> cleanRemoved = new List<string>();
                foreach (string r in lc.Removed)
                {
                    if (Contains(lc.BaselineEntries, r) && !Contains(cleanRemoved, r)) cleanRemoved.Add(r);
                }
                lc.Removed = cleanRemoved;
            }
            catch (Exception ex)
            {
                lc.LastError = ex.Message;
                Log.Error("Не удалось разобрать список " + Path.GetFileName(filePath), ex);
            }
            return lc;
        }

        public void SaveBaseline()
        {
            try
            {
                File.WriteAllText(BaselinePath(DataDir, FileName), string.Join("\r\n", BaselineEntries.ToArray()) + "\r\n", new UTF8Encoding(false));
            }
            catch (Exception ex) { Log.Error("Не удалось сохранить базовую версию " + FileName, ex); }
        }

        public void SaveChangesFile()
        {
            try
            {
                StringBuilder sb = new StringBuilder();
                sb.AppendLine("# Изменения пользователя в официальном списке " + FileName);
                sb.AppendLine("# + добавленные строки, - удалённые. Файл ведёт Zapret Manager.");
                foreach (string a in Added) sb.AppendLine("+ " + a);
                foreach (string r in Removed) sb.AppendLine("- " + r);
                File.WriteAllText(ChangesPath(DataDir, FileName), sb.ToString(), new UTF8Encoding(false));
            }
            catch (Exception ex) { Log.Error("Не удалось сохранить изменения " + FileName, ex); }
        }

        /// <summary>Текст файла с нашим блоком добавлений.</summary>
        public static string ComposeText(List<string> officialLines, List<string> added)
        {
            StringBuilder sb = new StringBuilder();
            foreach (string line in officialLines)
            {
                if (line.Trim().StartsWith("# >>> Zapret Manager", StringComparison.OrdinalIgnoreCase)) continue;
                if (line.Trim().StartsWith("# <<< Zapret Manager", StringComparison.OrdinalIgnoreCase)) continue;
                sb.AppendLine(line);
            }
            if (added.Count > 0)
            {
                while (sb.Length > 2 && (sb[sb.Length - 1] == '\n' || sb[sb.Length - 1] == '\r')) sb.Length--;
                sb.AppendLine();
                sb.AppendLine(BlockStart);
                foreach (string a in added) sb.AppendLine(a);
                sb.AppendLine(BlockEnd);
            }
            return sb.ToString();
        }

        /// <summary>
        /// Сохраняет отредактированный текст: новые строки помечаются как «мои добавления»,
        /// недостающие официальные — как «удалённые мной».
        /// </summary>
        public string SaveFromEditor(string editorText)
        {
            string[] lines = (editorText ?? "").Replace("\r\n", "\n").Replace("\r", "\n").Split('\n');
            List<string> officialLines;
            List<string> blockEntries;
            SplitBlock(lines, out officialLines, out blockEntries);

            List<string> asRead = ParseEntries(officialLines);
            List<string> baseline = BaselineEntries;
            List<string> added = new List<string>();
            foreach (string e in blockEntries) AddUnique(added, e);
            foreach (string e in asRead)
            {
                if (!Contains(baseline, e)) AddUnique(added, e);
            }

            List<string> removed = new List<string>();
            foreach (string e in baseline)
            {
                if (!Contains(asRead, e) && !Contains(added, e)) AddUnique(removed, e);
            }

            // строки, которые уже были в официальном списке, из блока убираем
            List<string> finalAdded = new List<string>();
            foreach (string a in added)
            {
                if (!Contains(baseline, a)) finalAdded.Add(a);
            }
            Added = finalAdded;
            Removed = removed;
            CurrentEntries = new List<string>(asRead);
            foreach (string a in finalAdded) AddUnique(CurrentEntries, a);

            // в файле оставляем только официальные строки: добавления уйдут под маркер
            List<string> officialOnly = new List<string>();
            foreach (string line in officialLines)
            {
                string trimmed = line.Trim();
                if (trimmed.Length == 0 || trimmed.StartsWith("#")) { officialOnly.Add(line); continue; }
                if (!Contains(baseline, line.Trim())) continue;   // это наше добавление — переносим в блок
                officialOnly.Add(line);
            }

            string text = ComposeText(officialOnly, finalAdded);
            try
            {
                Directory.CreateDirectory(Path.GetDirectoryName(FilePath));
                File.WriteAllText(FilePath, text, new UTF8Encoding(false));
                SaveChangesFile();
            }
            catch (Exception ex)
            {
                LastError = ex.Message;
                Log.Error("Не удалось сохранить список " + FileName, ex);
                return null;
            }
            return text;
        }

        /// <summary>
        /// Возвращает удалённые официальные строки в файл.
        /// Возвращает новый текст файла для редактора.
        /// </summary>
        public string RestoreRemoved(string editorText)
        {
            string[] lines = (editorText ?? "").Replace("\r\n", "\n").Split('\n');
            List<string> officialLines;
            List<string> blockEntries;
            SplitBlock(lines, out officialLines, out blockEntries);
            List<string> present = ParseEntries(officialLines);
            List<string> added = new List<string>(blockEntries);
            foreach (string e in present)
            {
                if (!Contains(BaselineEntries, e)) ListChanges.AddUnique(added, e);
            }
            // добавляем в конец официальной части то, что было удалено
            List<string> officialOnly = new List<string>();
            foreach (string line in officialLines)
            {
                string trimmed = line.Trim();
                if (trimmed.Length > 0 && !trimmed.StartsWith("#") && !Contains(BaselineEntries, trimmed)) continue;
                officialOnly.Add(line);
            }
            foreach (string e in BaselineEntries)
            {
                if (!Contains(present, e)) officialOnly.Add(e);
            }
            Removed = new List<string>();
            Added = new List<string>();
            foreach (string a in added)
            {
                if (!Contains(BaselineEntries, a)) ListChanges.AddUnique(Added, a);
            }
            CurrentEntries = ParseEntries(officialOnly);
            foreach (string a in Added) ListChanges.AddUnique(CurrentEntries, a);
            string text = ComposeText(officialOnly, Added);
            try
            {
                File.WriteAllText(FilePath, text, new UTF8Encoding(false));
                SaveChangesFile();
            }
            catch (Exception ex) { Log.Error("Не удалось вернуть удалённые строки " + FileName, ex); }
            return text;
        }

        /// <summary>Принимает текущее содержимое файла за официальную версию (стирает метки).</summary>
        public void AcceptAsOfficial(string editorText)
        {
            string[] lines = (editorText ?? "").Replace("\r\n", "\n").Split('\n');
            List<string> officialLines;
            List<string> blockEntries;
            SplitBlock(lines, out officialLines, out blockEntries);
            List<string> all = ParseEntries(officialLines);
            foreach (string b in blockEntries) AddUnique(all, b);
            BaselineEntries = all;
            Added = new List<string>();
            Removed = new List<string>();
            CurrentEntries = new List<string>(all);
            HasBaseline = true;
            SaveBaseline();
            SaveChangesFile();
            string text = ComposeText(officialLines, new List<string>());
            try { File.WriteAllText(FilePath, text, new UTF8Encoding(false)); } catch { }
        }

        /// <summary>
        /// Переносит «мои добавления» в личный список (-user.txt), который обновление не трогает.
        /// Возвращает количество перенесённых строк.
        /// </summary>
        public int MoveAddedToUserList(string userListPath)
        {
            if (Added.Count == 0) return 0;
            try
            {
                List<string> existing = File.Exists(userListPath)
                    ? ParseEntries(File.ReadAllLines(userListPath, Encoding.UTF8))
                    : new List<string>();
                List<string> toWrite = new List<string>();
                int moved = 0;
                foreach (string a in Added)
                {
                    if (!Contains(existing, a)) { toWrite.Add(a); moved++; }
                }
                if (toWrite.Count > 0)
                {
                    StringBuilder sb = new StringBuilder();
                    if (existing.Count == 0) sb.AppendLine("# Никогда не оставляйте этот файл пустым");
                    foreach (string e in existing) sb.AppendLine(e);
                    foreach (string e in toWrite) sb.AppendLine(e);
                    File.WriteAllText(userListPath, sb.ToString(), new UTF8Encoding(false));
                }
                // убираем перенесённые из официального списка
                List<string> remaining = new List<string>();
                foreach (string a in Added)
                {
                    if (!Contains(toWrite, a)) remaining.Add(a);
                }
                Added = remaining;
                SaveChangesFile();
                return moved;
            }
            catch (Exception ex)
            {
                Log.Error("Не удалось перенести добавления в личный список", ex);
                return 0;
            }
        }

        /// <summary>Применяет сохранённые изменения к новой официальной версии (после обновления).</summary>
        public static void ApplyAfterUpdate(ZapretInstall install, string dataDir, Action<string> log)
        {
            foreach (string name in OfficialFiles)
            {
                string path = Path.Combine(install.ListsDir, name);
                if (!File.Exists(path)) continue;
                ListChanges lc = new ListChanges();
                lc.FileName = name;
                lc.FilePath = path;
                lc.DataDir = dataDir;
                try
                {
                    string[] newLines = File.ReadAllLines(path, Encoding.UTF8);
                    List<string> newOfficial = ParseEntries(newLines);

                    // что нового принёс разработчик — фиксируем отдельно
                    List<string> oldBaseline = new List<string>();
                    string basePath = BaselinePath(dataDir, name);
                    if (File.Exists(basePath)) oldBaseline = ParseEntries(File.ReadAllLines(basePath, Encoding.UTF8));
                    List<string> news = new List<string>();
                    if (oldBaseline.Count > 0)
                    {
                        foreach (string e in newOfficial)
                        {
                            if (!Contains(oldBaseline, e)) news.Add(e);
                        }
                    }

                    // читаем пользовательские изменения
                    List<string> added = new List<string>();
                    List<string> removed = new List<string>();
                    string chPath = ChangesPath(dataDir, name);
                    if (File.Exists(chPath))
                    {
                        foreach (string raw in File.ReadAllLines(chPath, Encoding.UTF8))
                        {
                            string line = raw.Trim();
                            if (line.Length < 2) continue;
                            if (line[0] == '+') AddUnique(added, line.Substring(1).Trim());
                            else if (line[0] == '-') AddUnique(removed, line.Substring(1).Trim());
                        }
                    }

                    // к новой официальной версии применяем удаления и добавления
                    List<string> officialKept = new List<string>();
                    foreach (string line in newLines)
                    {
                        string trimmed = line.Trim();
                        if (trimmed.Length > 0 && !trimmed.StartsWith("#") && Contains(removed, trimmed)) continue;
                        officialKept.Add(line);
                    }
                    List<string> validAdded = new List<string>();
                    foreach (string a in added)
                    {
                        if (!Contains(newOfficial, a) && !Contains(validAdded, a)) validAdded.Add(a);
                    }
                    string text = ComposeText(officialKept, validAdded);
                    File.WriteAllText(path, text, new UTF8Encoding(false));

                    // новая база — официальная версия без наших добавлений
                    lc.BaselineEntries = newOfficial;
                    lc.HasBaseline = true;
                    lc.SaveBaseline();
                    lc.Added = validAdded;
                    lc.Removed = removed;
                    lc.SaveChangesFile();

                    Directory.CreateDirectory(ChangesDir(dataDir));
                    File.WriteAllText(NewsPath(dataDir, name), string.Join("\r\n", news.ToArray()), new UTF8Encoding(false));

                    if (added.Count > 0 || removed.Count > 0)
                    {
                        if (log != null) log(string.Format("{0}: сохранены ваши изменения (+{1} / −{2}), новых строк от разработчика: {3}",
                            name, validAdded.Count, removed.Count, news.Count));
                    }
                    else if (news.Count > 0 && log != null)
                    {
                        log(string.Format("{0}: новых строк от разработчика — {1}", name, news.Count));
                    }
                }
                catch (Exception ex)
                {
                    Log.Error("Не удалось применить изменения списка " + name, ex);
                }
            }
        }

        // ---------- утилиты ----------

        public static bool Contains(List<string> list, string value)
        {
            if (list == null || value == null) return false;
            foreach (string s in list)
            {
                if (string.Equals(s, value, StringComparison.OrdinalIgnoreCase)) return true;
            }
            return false;
        }

        public static void AddUnique(List<string> list, string value)
        {
            if (string.IsNullOrEmpty(value)) return;
            if (!Contains(list, value)) list.Add(value);
        }
    }
}
