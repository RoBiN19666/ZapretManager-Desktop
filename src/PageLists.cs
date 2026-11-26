using System;
using System.Collections.Generic;
using System.Drawing;
using System.IO;
using System.Text;
using System.Windows.Forms;

namespace ZapretManager
{
    public sealed class ListsPage : PageBase
    {
        private sealed class ListFile
        {
            public string Label;
            public string Path;
            public bool Official;
            public string Template;
            public string Hint;
        }

        private Card _card;
        private Card _changesCard;
        private DarkList _fileList;
        private DarkEditor _editor;
        private Label _info;
        private Label _pathLabel;
        private RoundButton _btnSave;
        private RoundButton _btnDedupe;
        private RoundButton _btnAdd;
        private RoundButton _btnNotepad;
        private RoundButton _btnReset;

        private Label _changesTitle;
        private Label _changesSummary;
        private DarkList _addedList;
        private DarkList _removedList;
        private Label _addedTitle;
        private Label _removedTitle;
        private RoundButton _btnRestore;
        private RoundButton _btnMove;
        private RoundButton _btnAccept;
        private List<ListFile> _files = new List<ListFile>();
        private ListChanges _changes;
        private bool _loading;
        private bool _dirty;
        private DateTime _ignoreTextUntil = DateTime.MinValue;
        private System.Windows.Forms.Timer _markTimer;

        public ListsPage(Shell shell) : base(shell) { }

        public override string Key { get { return "lists"; } }
        public override string Title { get { return "Списки"; } }
        public override string Subtitle { get { return "Домены и IP, к которым применяется обход"; } }
        public override string Icon { get { return "\uE8FD"; } }

        public override void Build()
        {
            _markTimer = new System.Windows.Forms.Timer();
            _markTimer.Interval = 400;
            _markTimer.Tick += delegate { _markTimer.Stop(); ApplyMarks(); };

            BuildEditorCard();
            BuildChangesCard();
            BuildFileList();
        }

        private void BuildEditorCard()
        {
            _card = NewCard("Файлы списков", 442);

            _fileList = new DarkList();
            _fileList.Location = new Point(20, 44);
            _fileList.Size = new Size(232, 292);
            _fileList.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Bottom;
            _fileList.SelectedIndexChanged += delegate { LoadSelected(); };
            _card.Controls.Add(_fileList);

            _pathLabel = new Label();
            _pathLabel.Location = new Point(268, 44);
            _pathLabel.Size = new Size(DesignWidth - 290, 16);
            _pathLabel.Font = Theme.Small;
            _pathLabel.ForeColor = Theme.TextMuted;
            _pathLabel.BackColor = Color.Transparent;
            _pathLabel.AutoSize = false;
            _pathLabel.AutoEllipsis = true;
            _pathLabel.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right;
            _card.Controls.Add(_pathLabel);

            _editor = new DarkEditor();
            _editor.Location = new Point(268, 64);
            _editor.Size = new Size(DesignWidth - 288, 252);
            _editor.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right | AnchorStyles.Bottom;
            _editor.TextChanged += delegate
            {
                if (_loading || DateTime.Now < _ignoreTextUntil) return;
                _dirty = true;
                UpdateInfo();
                _markTimer.Stop();
                _markTimer.Start();
            };
            _card.Controls.Add(_editor);

            int y = 332;
            _btnSave = Btn("Сохранить", ButtonKind.Primary, 116);
            _btnSave.Location = new Point(268, y);
            _btnSave.Anchor = AnchorStyles.Left | AnchorStyles.Bottom;
            _btnSave.Click += delegate { SaveSelected(); };
            _card.Controls.Add(_btnSave);

            _btnAdd = Btn("Добавить строку", ButtonKind.Secondary, 150);
            _btnAdd.Location = new Point(392, y);
            _btnAdd.Anchor = AnchorStyles.Left | AnchorStyles.Bottom;
            _btnAdd.Click += delegate { AddEmptyLine(); };
            _card.Controls.Add(_btnAdd);

            _btnNotepad = Btn("Открыть в блокноте", ButtonKind.Ghost, 178);
            _btnNotepad.Location = new Point(550, y);
            _btnNotepad.Anchor = AnchorStyles.Left | AnchorStyles.Bottom;
            _btnNotepad.Click += delegate { OpenNotepad(); };
            _card.Controls.Add(_btnNotepad);

            _btnReset = Btn("Сброс к шаблону", ButtonKind.Danger, 144);
            _btnReset.Location = new Point(736, y);
            _btnReset.Anchor = AnchorStyles.Left | AnchorStyles.Bottom;
            _btnReset.Click += delegate { ResetToTemplate(); };
            _card.Controls.Add(_btnReset);

            _info = L("", Theme.Small, Theme.TextMuted, 268, y + 38);
            _card.Controls.Add(_info);

            _btnDedupe = Btn("Убрать дубликаты", ButtonKind.Link, 160);
            _btnDedupe.Location = new Point(268, y + 64);
            _btnDedupe.Anchor = AnchorStyles.Left | AnchorStyles.Bottom;
            _btnDedupe.Visible = false;
            _btnDedupe.Click += delegate { DedupeNow(); };
            _card.Controls.Add(_btnDedupe);

            _card.Resize += delegate
            {
                int w = _card.ClientSize.Width;
                _editor.Size = new Size(w - 288, Math.Max(80, _card.ClientSize.Height - 190));
                _fileList.Height = Math.Max(80, _card.ClientSize.Height - 108);
            };

            View.Stack.Add(_card);
        }

        private void BuildChangesCard()
        {
            _changesCard = NewCard("Мои изменения в официальном списке", 266);
            _changesCard.AccentStripe = Theme.Success;

            _changesTitle = L("", Theme.Small, Theme.TextMuted, 22, 40);
            _changesCard.Controls.Add(_changesTitle);

            _changesSummary = L("", Theme.Base, Theme.Text, 22, 60);
            _changesCard.Controls.Add(_changesSummary);

            _addedTitle = L("Добавлено вами (0)", Theme.Small, Theme.Success, 22, 88);
            _changesCard.Controls.Add(_addedTitle);
            _addedList = new DarkList();
            _addedList.Location = new Point(22, 108);
            _addedList.Size = new Size(400, 96);
            _addedList.Anchor = AnchorStyles.Top | AnchorStyles.Left;
            _changesCard.Controls.Add(_addedList);

            _removedTitle = L("Удалено вами (0)", Theme.Small, Theme.Danger, 438, 88);
            _changesCard.Controls.Add(_removedTitle);
            _removedList = new DarkList();
            _removedList.Location = new Point(438, 108);
            _removedList.Size = new Size(DesignWidth - 460, 96);
            _removedList.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right;
            _changesCard.Controls.Add(_removedList);

            int y = 210;
            _btnRestore = Btn("Вернуть удалённые", ButtonKind.Secondary, 170);
            _btnRestore.Location = new Point(22, y);
            _btnRestore.Click += delegate { RestoreRemoved(); };
            _changesCard.Controls.Add(_btnRestore);

            _btnMove = Btn("Перенести в мой список", ButtonKind.Secondary, 200);
            _btnMove.Location = new Point(202, y);
            _btnMove.Click += delegate { MoveToUserList(); };
            _changesCard.Controls.Add(_btnMove);

            _btnAccept = Btn("Принять как официальное", ButtonKind.Ghost, 210);
            _btnAccept.Location = new Point(412, y);
            _btnAccept.Click += delegate { AcceptAsOfficial(); };
            _changesCard.Controls.Add(_btnAccept);

            _changesCard.Controls.Add(L("Строки «+» переносятся при обновлении zapret, «−» — снова удаляются. Новые строки разработчика при этом не теряются.",
                Theme.Small, Theme.TextMuted, 22, y + 38));
            View.Stack.Add(_changesCard);
        }

        private void BuildFileList()
        {
            _files.Clear();
            if (Shell.Install == null || !Shell.Install.IsValid)
            {
                _editor.Text = "";
                _info.Text = "Папка zapret не найдена.";
                _changesCard.Visible = false;
                return;
            }
            string lists = Shell.Install.ListsDir;
            AddFile("list-general-user.txt", "Мои домены", Path.Combine(lists, "list-general-user.txt"), false,
                "# Никогда не оставляйте этот файл пустым\r\ndomain.example.abc\r\n", "Домены, которые нужно обходить (добавляются к официальному списку)");
            AddFile("list-exclude-user.txt", "Исключения", Path.Combine(lists, "list-exclude-user.txt"), false,
                "domain.example.abc\r\n", "Домены, которые обходить НЕ нужно (перекрывают оба списка)");
            AddFile("ipset-exclude-user.txt", "Исключения IP", Path.Combine(lists, "ipset-exclude-user.txt"), false,
                "203.0.113.113/32\r\n", "IP-подсети, которые обходить не нужно");
            AddFile("list-general.txt", "Официальный список", Path.Combine(lists, "list-general.txt"), true, "",
                "Основной список доменов из репозитория (обновляется вместе с zapret)");
            AddFile("list-google.txt", "Google / YouTube", Path.Combine(lists, "list-google.txt"), true, "",
                "Список доменов Google и YouTube");
            AddFile("list-exclude.txt", "Официальные исключения", Path.Combine(lists, "list-exclude.txt"), true, "",
                "Исключения из репозитория (банки, игры, обновления)");
            AddFile("ipset-exclude.txt", "Официальные IP-исключения", Path.Combine(lists, "ipset-exclude.txt"), true, "",
                "Локальные и служебные подсети из репозитория");
            AddFile("ipset-all.txt", "IP-список (рабочий)", Shell.Install.IpsetFile, true, "",
                "Текущий IP-лист: полный список, пустой файл или заглушка");

            _fileList.Items.Clear();
            List<string> subs = new List<string>();
            string[] names = new string[_files.Count];
            for (int i = 0; i < _files.Count; i++)
            {
                names[i] = _files[i].Label;
                subs.Add(_files[i].Official ? "официальный" : "мой список");
            }
            _fileList.Subtitles = subs.ToArray();
            _fileList.Items.AddRange(names);
            if (_fileList.Items.Count > 0) _fileList.SelectedIndex = 0;
        }

        private void AddFile(string fileName, string label, string path, bool official, string template, string hint)
        {
            ListFile f = new ListFile();
            f.Label = label;
            f.Path = path;
            f.Official = official;
            f.Template = template;
            f.Hint = hint;
            _files.Add(f);
        }

        private ListFile SelectedFile()
        {
            int i = _fileList.SelectedIndex;
            if (i < 0 || i >= _files.Count) return null;
            return _files[i];
        }

        private void LoadSelected()
        {
            ListFile f = SelectedFile();
            if (f == null) return;
            _loading = true;
            _changes = null;
            try
            {
                _pathLabel.Text = f.Path + "  —  " + f.Hint;
                try
                {
                    if (File.Exists(f.Path)) _editor.Text = File.ReadAllText(f.Path, Encoding.UTF8).Replace("\r\n", "\n").Replace("\n", "\r\n");
                    else _editor.Text = f.Official ? "" : (f.Template ?? "");
                }
                catch (Exception ex)
                {
                    _editor.Text = "";
                    _info.Text = "Не удалось прочитать файл: " + ex.Message;
                }
                _dirty = false;
            }
            finally { _loading = false; }

            bool markable = f.Official && ListChanges.IsOfficial(Path.GetFileName(f.Path));
            if (markable)
            {
                _changes = ListChanges.Load(f.Path, AppPaths.DataDir);
                if (_changes.LastError.Length > 0) Shell.Notify("Не удалось разобрать изменения: " + _changes.LastError, LogLevel.Warn);
            }
            _ignoreTextUntil = DateTime.Now.AddMilliseconds(800);
            _dirty = false;
            _btnSave.Enabled = true;
            _btnReset.Enabled = !f.Official;
            _btnAdd.Enabled = true;
            _changesCard.Visible = markable;
            UpdateInfo();
            UpdateChangesCard();
            ApplyMarks();
            if (_changesCard.Visible) View.Stack.PerformLayout();
        }

        private void UpdateInfo()
        {
            ListFile f = SelectedFile();
            if (f == null) return;
            int lines = 0;
            int entries = 0;
            string[] raw = _editor.Text.Replace("\r\n", "\n").Split('\n');
            foreach (string line in raw)
            {
                if (line.Trim().Length == 0) continue;
                lines++;
                if (!line.TrimStart().StartsWith("#")) entries++;
            }
            string extra = "";
            if (!f.Official && entries == 0) extra = "  •  файл пуст — так нельзя для списка доменов";

            List<ListNote> notes = ListAudit.Analyze(_editor.Text);
            int dupes = ListAudit.CountKind(notes, "dup");
            int wild = ListAudit.CountKind(notes, "wildcard");
            string issues = "";
            if (dupes > 0) issues += string.Format("  •  дубликатов: {0}", dupes);
            if (wild > 0) issues += string.Format("  •  «*.» не поддерживается: {0}", wild);

            string state = _dirty ? "Есть несохранённые изменения" : "Сохранено";
            _info.Text = string.Format("{0}  •  строк: {1}, записей: {2}{3}{4}", state, lines, entries, extra, issues);
            _info.ForeColor = (entries == 0 && !f.Official) || dupes > 0 || wild > 0 ? Theme.Warn : Theme.TextMuted;
            if (_btnDedupe != null) _btnDedupe.Visible = dupes > 0;
        }

        // ---------- разметка добавленных строк ----------

        private void ApplyMarks()
        {
            if (_changes == null || _editor.TextLength == 0) return;
            try
            {
                int caret = _editor.SelectionStart;
                int selLen = _editor.SelectionLength;
                Native.SetRedraw(_editor.Handle, false);
                _editor.SuspendLayout();
                string text = _editor.Text;
                string[] lines = text.Split('\n');
                List<string> baseline = _changes.BaselineEntries;
                bool inBlock = false;
                HashSet<string> duplicated = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
                HashSet<string> seenEntries = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
                foreach (ListNote note in ListAudit.Analyze(text))
                {
                    if (note.Kind == "dup") duplicated.Add(note.Entry);
                }
                int pos = 0;
                for (int i = 0; i < lines.Length; i++)
                {
                    string raw = lines[i];
                    int len = raw.Length;
                    string trimmed = raw.Trim();
                    if (trimmed.StartsWith("# >>> Zapret Manager", StringComparison.OrdinalIgnoreCase)) inBlock = true;
                    Color color = Theme.Text;
                    if (inBlock || (trimmed.Length > 0 && !trimmed.StartsWith("#") && !ListChanges.Contains(baseline, trimmed)))
                    {
                        color = Color.FromArgb(140, 226, 170);   // добавлено вами
                    }
                    string norm = ListAudit.Normalize(trimmed);
                    if (norm.StartsWith("*."))
                    {
                        color = Color.FromArgb(255, 194, 74);    // недопустимая звёздочка
                    }
                    if (norm.Length > 0 && duplicated.Contains(norm))
                    {
                        color = Color.FromArgb(255, 120, 105);   // повтор
                    }
                    else if (trimmed.StartsWith("#"))
                    {
                        color = Theme.TextMuted;
                    }
                    if (len > 0)
                    {
                        _editor.Select(pos, len);
                        _editor.SelectionColor = color;
                    }
                    pos += len + 1;
                    if (trimmed.StartsWith("# <<< Zapret Manager", StringComparison.OrdinalIgnoreCase)) inBlock = false;
                }
                _editor.Select(Math.Min(caret, _editor.TextLength), Math.Min(selLen, Math.Max(0, _editor.TextLength - caret)));
                _editor.SelectionColor = Theme.Text;
                _editor.ResumeLayout();
                Native.SetRedraw(_editor.Handle, true);
                _editor.Invalidate();
            }
            catch (Exception ex) { Log.Warn("Не удалось разметить список: " + ex.Message); }
        }

        private void UpdateChangesCard()
        {
            ListFile f = SelectedFile();
            if (f == null || _changes == null) return;
            _changesTitle.Text = "Файл: " + Path.GetFileName(f.Path) + "  •  строк официальной версии: " + _changes.BaselineEntries.Count;
            _changesSummary.Text = string.Format("Добавлено вами: {0}   •   удалено вами: {1}   •   новых строк от разработчика в последней версии: {2}",
                _changes.Added.Count, _changes.Removed.Count, _changes.News.Count);
            _changesSummary.ForeColor = (_changes.Added.Count + _changes.Removed.Count) > 0 ? Theme.Text : Theme.TextDim;

            _addedTitle.Text = string.Format("Добавлено вами ({0})", _changes.Added.Count);
            _removedTitle.Text = string.Format("Удалено вами ({0})", _changes.Removed.Count);

            FillList(_addedList, _changes.Added);
            FillList(_removedList, _changes.Removed);

            _btnRestore.Enabled = _changes.Removed.Count > 0;
            string userList = UserListFor(Path.GetFileName(f.Path));
            _btnMove.Enabled = _changes.Added.Count > 0 && userList != null;
            _btnAccept.Enabled = _changes.Added.Count > 0 || _changes.Removed.Count > 0;
        }

        private static void FillList(DarkList list, List<string> items)
        {
            list.BeginUpdate();
            list.Items.Clear();
            if (items.Count == 0)
            {
                list.Subtitles = new string[] { "пусто" };
                list.Items.Add("—");
            }
            else
            {
                list.Subtitles = new string[items.Count];
                for (int i = 0; i < items.Count; i++)
                {
                    list.Subtitles[i] = "";
                    list.Items.Add(items[i]);
                }
            }
            list.EndUpdate();
        }

        private static string UserListFor(string officialName)
        {
            if (string.Equals(officialName, "list-general.txt", StringComparison.OrdinalIgnoreCase)) return "list-general-user.txt";
            if (string.Equals(officialName, "list-exclude.txt", StringComparison.OrdinalIgnoreCase)) return "list-exclude-user.txt";
            if (string.Equals(officialName, "ipset-exclude.txt", StringComparison.OrdinalIgnoreCase)) return "ipset-exclude-user.txt";
            return null;
        }

        // ---------- действия ----------

        private void SaveSelected()
        {
            ListFile f = SelectedFile();
            if (f == null) return;

            // Чистим список перед сохранением: «*.домен» → «домен», повторы — убираем
            int fixedWild, removedDup;
            string prepared = ListAudit.FixWildcards(_editor.Text, out fixedWild);
            prepared = ListAudit.RemoveDuplicates(prepared, out removedDup);
            if (fixedWild > 0 || removedDup > 0)
            {
                _loading = true;
                _editor.Text = prepared.Replace("\r\n", "\n").Replace("\n", "\r\n");
                _loading = false;
                _dirty = true;
                ApplyMarks();
                UpdateInfo();
                Shell.Notify(string.Format("Список почищен: убрано повторов {0}, заменено «*.» {1}.", removedDup, fixedWild), LogLevel.Ok);
            }
            if (f.Official && _changes == null)
            {
                Shell.Notify("Официальные списки можно править — изменения будут помечены. Перезагрузите страницу.", LogLevel.Warn);
                return;
            }
            try
            {
                if (f.Official)
                {
                    string text = _changes.SaveFromEditor(_editor.Text);
                    if (text == null) { Shell.Notify("Не удалось сохранить: " + _changes.LastError, LogLevel.Error); return; }
                    _loading = true;
                    _editor.Text = text.Replace("\r\n", "\n").Replace("\n", "\r\n");
                    _loading = false;
                    _dirty = false;
                    _changes = ListChanges.Load(f.Path, AppPaths.DataDir);
                    ApplyMarks();
                    UpdateChangesCard();
                    UpdateInfo();
                    Shell.Notify(string.Format("Сохранено. Ваши добавления ({0}) помечены и переживут обновление zapret.", _changes.Added.Count), LogLevel.Ok);
                    return;
                }

                int entries = 0;
                foreach (string line in _editor.Text.Replace("\r\n", "\n").Split('\n'))
                {
                    if (line.Trim().Length > 0 && !line.TrimStart().StartsWith("#")) entries++;
                }
                if (entries == 0 && f.Path.EndsWith("list-general-user.txt", StringComparison.OrdinalIgnoreCase))
                {
                    Shell.Notify("Список «Мои домены» нельзя сохранить пустым — winws.exe не запустится.", LogLevel.Error);
                    return;
                }
                Directory.CreateDirectory(Path.GetDirectoryName(f.Path));
                File.WriteAllText(f.Path, _editor.Text.Replace("\r\n", "\n").Replace("\n", "\r\n"), new UTF8Encoding(false));
                _dirty = false;
                UpdateInfo();
                Shell.Notify("Список сохранён: " + Path.GetFileName(f.Path) + ". Перезапустите bypass, чтобы применить.", LogLevel.Ok);
            }
            catch (Exception ex)
            {
                Shell.Notify("Не удалось сохранить список: " + ex.Message, LogLevel.Error);
            }
        }

        private void RestoreRemoved()
        {
            if (_changes == null) return;
            if (!Dlg.Confirm(this.View.FindForm(), "Вернуть удалённые строки?",
                string.Format("{0} строк официального списка будут возвращены в файл.", _changes.Removed.Count), "Вернуть")) return;
            string text = _changes.RestoreRemoved(_editor.Text);
            _loading = true;
            _editor.Text = text.Replace("\r\n", "\n").Replace("\n", "\r\n");
            _loading = false;
            _changes = ListChanges.Load(SelectedFile().Path, AppPaths.DataDir);
            ApplyMarks();
            UpdateChangesCard();
            Shell.Notify("Удалённые строки возвращены.", LogLevel.Ok);
        }

        private void MoveToUserList()
        {
            ListFile f = SelectedFile();
            if (f == null || _changes == null) return;
            string userList = UserListFor(Path.GetFileName(f.Path));
            if (userList == null)
            {
                Shell.Notify("Для этого списка нет пользовательского аналога — оставьте строки в размеченном блоке.", LogLevel.Warn);
                return;
            }
            string target = Path.Combine(Shell.Install.ListsDir, userList);
            if (!Dlg.Confirm(this.View.FindForm(), "Перенести добавления?",
                string.Format("Ваши строки ({0}) будут перенесены в {1} — этот файл обновление никогда не перезаписывает. Из официального списка они исчезнут.",
                    _changes.Added.Count, userList), "Перенести")) return;
            int moved = _changes.MoveAddedToUserList(target);
            string text = ListChanges.ComposeText(SplitOfficial(_editor.Text), new List<string>());
            try { File.WriteAllText(f.Path, text, new UTF8Encoding(false)); } catch { }
            _loading = true;
            _editor.Text = text.Replace("\r\n", "\n").Replace("\n", "\r\n");
            _loading = false;
            _changes = ListChanges.Load(f.Path, AppPaths.DataDir);
            ApplyMarks();
            UpdateChangesCard();
            Shell.Notify(string.Format("Перенесено строк: {0} → {1}. Перезапустите bypass.", moved, userList), LogLevel.Ok);
        }

        private static List<string> SplitOfficial(string text)
        {
            string[] lines = text.Replace("\r\n", "\n").Split('\n');
            List<string> off;
            List<string> block;
            ListChanges.SplitBlock(lines, out off, out block);
            if (off.Count > 0 && off[off.Count - 1].Trim().Length == 0) off.RemoveAt(off.Count - 1);
            return off;
        }

        private void AcceptAsOfficial()
        {
            ListFile f = SelectedFile();
            if (f == null || _changes == null) return;
            if (!Dlg.ConfirmDanger(this.View.FindForm(), "Принять как официальное?",
                "Текущее содержимое станет «официальной версией»: пометки «добавлено/удалено» сбросятся, и при следующем обновлении эти строки больше не будут переноситься отдельно.",
                "Принять")) return;
            _changes.AcceptAsOfficial(_editor.Text);
            _loading = true;
            _editor.Text = File.ReadAllText(f.Path, Encoding.UTF8).Replace("\r\n", "\n").Replace("\n", "\r\n");
            _loading = false;
            _changes = ListChanges.Load(f.Path, AppPaths.DataDir);
            ApplyMarks();
            UpdateChangesCard();
            Shell.Notify("Текущее содержимое зафиксировано как официальное.", LogLevel.Ok);
        }

        private void DedupeNow()
        {
            int removed;
            string text = ListAudit.RemoveDuplicates(_editor.Text, out removed);
            if (removed == 0)
            {
                Shell.Notify("Повторов нет.", LogLevel.Ok);
                return;
            }
            _loading = true;
            _editor.Text = text.Replace("\r\n", "\n").Replace("\n", "\r\n");
            _loading = false;
            _dirty = true;
            ApplyMarks();
            UpdateInfo();
            Shell.Notify(string.Format("Удалено повторов: {0}. Не забудьте сохранить.", removed), LogLevel.Ok);
        }

        private void AddEmptyLine()
        {
            int caret = _editor.SelectionStart;
            string insert = (caret > 0 ? "\r\n" : "") + "domain.example.com";
            _editor.Text = _editor.Text.Substring(0, caret) + insert + _editor.Text.Substring(caret);
            _editor.SelectionStart = caret + insert.Length;
            _editor.Focus();
        }

        private void OpenNotepad()
        {
            ListFile f = SelectedFile();
            if (f == null) return;
            try
            {
                if (!File.Exists(f.Path)) File.WriteAllText(f.Path, f.Template ?? "", new UTF8Encoding(false));
                System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo("notepad.exe", "\"" + f.Path + "\"") { UseShellExecute = true });
            }
            catch (Exception ex) { Shell.Notify("Не удалось открыть блокнот: " + ex.Message, LogLevel.Error); }
        }

        private void ResetToTemplate()
        {
            ListFile f = SelectedFile();
            if (f == null || f.Official) return;
            if (!Dlg.ConfirmDanger(this.View.FindForm(), "Сбросить файл?",
                string.Format("Содержимое «{0}» будет заменено шаблоном по умолчанию.", f.Label), "Сбросить")) return;
            _editor.Text = f.Template ?? "";
            SaveSelected();
        }

        public override void OnShow()
        {
            if (_files.Count == 0 && Shell.Install != null && Shell.Install.IsValid) BuildFileList();
            if (!string.IsNullOrEmpty(AppInfo.ShotList))
            {
                for (int i = 0; i < _files.Count; i++)
                {
                    if (string.Equals(Path.GetFileName(_files[i].Path), AppInfo.ShotList, StringComparison.OrdinalIgnoreCase))
                    {
                        _fileList.SelectedIndex = i;
                        break;
                    }
                }
            }
            RefreshState();
        }

        public override void RefreshState()
        {
        }
    }
}
