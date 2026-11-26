using System;
using System.Collections.Generic;
using System.Text;

namespace ZapretManager
{
    public sealed class ListNote
    {
        public int Line;          // 0-based индекс строки в тексте
        public string Entry = ""; // нормализованная запись
        public string Kind = "";  // dup | wildcard
    }

    /// <summary>
    /// Проверки для списков zapret: дубликаты и недопустимые «*.домен».
    /// В hostlist поддомены учитываются автоматически (см. docs zapret),
    /// поэтому звёздочка не нужна и не работает — она попадает в файл как есть.
    /// </summary>
    public static class ListAudit
    {
        public static List<ListNote> Analyze(string text)
        {
            List<ListNote> notes = new List<ListNote>();
            if (string.IsNullOrEmpty(text)) return notes;
            string[] lines = text.Replace("\r\n", "\n").Split('\n');
            Dictionary<string, int> seen = new Dictionary<string, int>(StringComparer.OrdinalIgnoreCase);
            for (int i = 0; i < lines.Length; i++)
            {
                string entry = Normalize(lines[i]);
                if (entry.Length == 0) continue;

                if (entry.StartsWith("*."))
                {
                    ListNote w = new ListNote();
                    w.Line = i;
                    w.Entry = entry;
                    w.Kind = "wildcard";
                    notes.Add(w);
                }

                int first;
                if (seen.TryGetValue(entry, out first))
                {
                    ListNote d = new ListNote();
                    d.Line = i;
                    d.Entry = entry;
                    d.Kind = "dup";
                    notes.Add(d);
                }
                else
                {
                    seen[entry] = i;
                }
            }
            return notes;
        }

        public static int CountKind(List<ListNote> notes, string kind)
        {
            int n = 0;
            foreach (ListNote note in notes) if (note.Kind == kind) n++;
            return n;
        }

        /// <summary>Убирает повторные записи, сохраняя комментарии и порядок.</summary>
        public static string RemoveDuplicates(string text, out int removed)
        {
            removed = 0;
            if (string.IsNullOrEmpty(text)) return text;
            bool endsWithNewLine = text.EndsWith("\n");
            string[] lines = text.Replace("\r\n", "\n").Split('\n');
            List<string> result = new List<string>();
            HashSet<string> seen = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
            foreach (string line in lines)
            {
                string entry = Normalize(line);
                if (entry.Length == 0)
                {
                    result.Add(line);
                    continue;
                }
                if (!seen.Add(entry))
                {
                    removed++;
                    continue;
                }
                result.Add(line);
            }
            string joined = string.Join("\n", result.ToArray());
            if (!endsWithNewLine && joined.EndsWith("\n")) joined = joined.TrimEnd('\n');
            return joined;
        }

        /// <summary>Заменяет «*.домен» на «домен» — так поддомены и так попадают в список.</summary>
        public static string FixWildcards(string text, out int fixedCount)
        {
            fixedCount = 0;
            if (string.IsNullOrEmpty(text)) return text;
            string[] lines = text.Replace("\r\n", "\n").Split('\n');
            for (int i = 0; i < lines.Length; i++)
            {
                string raw = lines[i];
                int pos = 0;
                while (pos < raw.Length && char.IsWhiteSpace(raw[pos])) pos++;
                string body = raw.Substring(pos);
                if (body.StartsWith("*.") && body.Length > 2)
                {
                    lines[i] = raw.Substring(0, pos) + body.Substring(2);
                    fixedCount++;
                }
            }
            return string.Join("\n", lines);
        }

        /// <summary>Запись без комментариев, пробелов и с нижним регистром.</summary>
        public static string Normalize(string line)
        {
            if (line == null) return "";
            string t = line.Trim();
            if (t.Length == 0) return "";
            // в zapret комментарии начинаются с #, ; или /
            if (t[0] == '#' || t[0] == ';' || t[0] == '/') return "";
            return t.ToLowerInvariant();
        }

        public static bool IsComment(string line)
        {
            string t = (line ?? "").TrimStart();
            return t.Length > 0 && (t[0] == '#' || t[0] == ';' || t[0] == '/');
        }

        /// <summary>
        /// Разбирает вставленный текст: домены через запятую, точку с запятой, пробел, таб, перенос строки.
        /// Убирает ссылки, пути, порты, кавычки, «*.» и мусор. Возвращает чистый список без повторов.
        /// </summary>
        public static System.Collections.Generic.List<string> ParseBatch(string text, out int skipped)
        {
            System.Collections.Generic.List<string> result = new System.Collections.Generic.List<string>();
            skipped = 0;
            if (string.IsNullOrEmpty(text)) return result;
            string[] tokens = System.Text.RegularExpressions.Regex.Split(text, "[\\s,;|\\t\\r\\n]+");
            foreach (string raw in tokens)
            {
                string host = CleanToken(raw);
                if (host.Length == 0)
                {
                    if ((raw ?? "").Trim().Length > 0) skipped++;
                    continue;
                }
                if (!IsHostLike(host))
                {
                    skipped++;
                    continue;
                }
                bool exists = false;
                foreach (string item in result)
                {
                    if (string.Equals(item, host, StringComparison.OrdinalIgnoreCase)) { exists = true; break; }
                }
                if (exists) { skipped++; continue; }
                result.Add(host);
            }
            return result;
        }

        /// <summary>Приводит строку к домену: убирает кавычки, схему, путь, порт, звёздочку.</summary>
        public static string CleanToken(string raw)
        {
            if (raw == null) return "";
            string t = raw.Trim().Trim('"', '\'', '`', '<', '>', '(', ')', '[', ']', ',', ';', '|');
            if (t.Length == 0) return "";
            t = t.Replace("\\", "/");
            int scheme = t.IndexOf("://");
            if (scheme >= 0) t = t.Substring(scheme + 3);
            else if (t.StartsWith("//")) t = t.Substring(2);
            int slash = t.IndexOfAny(new char[] { '/', '?', '#' });
            if (slash >= 0) t = t.Substring(0, slash);
            int at = t.IndexOf('@');
            if (at >= 0) t = t.Substring(at + 1);
            if (t.StartsWith("*.")) t = t.Substring(2);
            if (t.StartsWith("^")) t = t.Substring(1);
            int colon = t.IndexOf(':');
            if (colon > 0) t = t.Substring(0, colon);
            t = t.TrimEnd('.');
            t = t.ToLowerInvariant();
            return t;
        }

        /// <summary>Похоже ли на домен или IP.</summary>
        public static bool IsHostLike(string host)
        {
            if (string.IsNullOrEmpty(host) || host.Length > 253) return false;
            System.Net.IPAddress ip;
            if (System.Net.IPAddress.TryParse(host, out ip)) return true;
            if (host.IndexOf('.') < 0) return false;
            if (host.StartsWith(".") || host.EndsWith(".") || host.Contains("..")) return false;
            string[] labels = host.Split('.');
            for (int i = 0; i < labels.Length; i++)
            {
                string label = labels[i];
                if (label.Length == 0 || label.Length > 63) return false;
                foreach (char ch in label)
                {
                    bool ok = (ch >= 'a' && ch <= 'z') || (ch >= '0' && ch <= '9') || ch == '-';
                    if (!ok) return false;
                }
                if (label.StartsWith("-") || label.EndsWith("-")) return false;
            }
            string tld = labels[labels.Length - 1];
            if (tld.Length < 2) return false;
            foreach (char ch in tld)
            {
                if (!(ch >= 'a' && ch <= 'z')) return false;
            }
            return true;
        }

        /// <summary>Чистит все строки текста; комментарии и пустые строки сохраняются.</summary>
        public static string Tidy(string text, out int changed, out int removed)
        {
            changed = 0;
            removed = 0;
            if (string.IsNullOrEmpty(text)) return text;
            string[] lines = text.Replace("\r\n", "\n").Split('\n');
            System.Collections.Generic.List<string> outLines = new System.Collections.Generic.List<string>();
            System.Collections.Generic.HashSet<string> seen = new System.Collections.Generic.HashSet<string>(StringComparer.OrdinalIgnoreCase);
            foreach (string line in lines)
            {
                if (IsComment(line) || line.Trim().Length == 0)
                {
                    outLines.Add(line);
                    continue;
                }
                string host = CleanToken(line);
                if (host.Length == 0 || !IsHostLike(host))
                {
                    removed++;
                    continue;
                }
                if (!seen.Add(host))
                {
                    removed++;
                    continue;
                }
                if (!string.Equals(host, line.Trim(), StringComparison.Ordinal)) changed++;
                outLines.Add(host);
            }
            return string.Join("\n", outLines.ToArray());
        }
    }
}

