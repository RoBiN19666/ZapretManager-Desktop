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
    }
}
