using System;
using System.Collections.Generic;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Drawing.Text;
using System.IO;
using System.Windows.Forms;

namespace ZapretManager
{
    /// <summary>Таблица результатов прогона стратегий.</summary>
    public class TestGrid : Control
    {
        public List<StrategyTestRow> Rows = new List<StrategyTestRow>();
        public List<string> Groups = new List<string>();
        public int SelectedIndex = -1;
        public event EventHandler SelectedIndexChanged;
        public int BestScore = 0;

        private int _hover = -1;
        public const int RowH = 34;
        public const int HeadH = 36;

        public TestGrid()
        {
            SetStyle(ControlStyles.AllPaintingInWmPaint | ControlStyles.UserPaint | ControlStyles.OptimizedDoubleBuffer | ControlStyles.ResizeRedraw, true);
            BackColor = Theme.Surface;
            Font = Theme.Base;
            Cursor = Cursors.Hand;
            Height = HeadH + 10;
        }

        public void UpdateHeight()
        {
            Height = HeadH + Math.Max(1, Rows.Count) * RowH + 6;
        }

        private int NameWidth
        {
            get { return Math.Max(180, (int)((ClientSize.Width - 110) * 0.34)); }
        }

        private int GroupWidth
        {
            get
            {
                int n = Math.Max(1, Groups.Count);
                return Math.Max(60, (ClientSize.Width - NameWidth - 100) / n);
            }
        }

        protected override void OnMouseMove(MouseEventArgs e)
        {
            base.OnMouseMove(e);
            int idx = RowAt(e.Y);
            if (idx != _hover) { _hover = idx; Invalidate(); }
        }

        protected override void OnMouseLeave(EventArgs e)
        {
            base.OnMouseLeave(e);
            _hover = -1;
            Invalidate();
        }

        private int RowAt(int y)
        {
            if (y < HeadH) return -1;
            int idx = (y - HeadH) / RowH;
            return (idx >= 0 && idx < Rows.Count) ? idx : -1;
        }

        protected override void OnMouseDown(MouseEventArgs e)
        {
            base.OnMouseDown(e);
            int idx = RowAt(e.Y);
            if (idx >= 0)
            {
                SelectedIndex = idx;
                Invalidate();
                EventHandler h = SelectedIndexChanged;
                if (h != null) h(this, EventArgs.Empty);
            }
        }

        protected override void OnPaint(PaintEventArgs e)
        {
            Graphics g = e.Graphics;
            g.SmoothingMode = SmoothingMode.AntiAlias;
            using (SolidBrush b = new SolidBrush(Theme.Surface)) g.FillRectangle(b, ClientRectangle);

            // Заголовок
            Draw.FillRound(g, new Rectangle(4, 4, ClientSize.Width - 8, HeadH - 8), 8, Theme.SurfaceAlt);
            int x = 18;
            TextRenderer.DrawText(g, "Стратегия", Theme.Bold, new Rectangle(x, 4, NameWidth - 20, HeadH), Theme.TextDim,
                TextFormatFlags.Left | TextFormatFlags.VerticalCenter | TextFormatFlags.NoPrefix);
            int gx = NameWidth;
            foreach (string gr in Groups)
            {
                TextRenderer.DrawText(g, gr, Theme.Small, new Rectangle(gx, 4, GroupWidth, HeadH), Theme.TextDim,
                    TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter | TextFormatFlags.NoPrefix | TextFormatFlags.EndEllipsis);
                gx += GroupWidth;
            }
            TextRenderer.DrawText(g, "Итог", Theme.Small, new Rectangle(ClientSize.Width - 100, 4, 88, HeadH), Theme.TextDim,
                TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter | TextFormatFlags.NoPrefix);

            if (Rows.Count == 0)
            {
                TextRenderer.DrawText(g, "Результатов пока нет — запустите тест", Theme.Base,
                    new Rectangle(0, HeadH + 10, ClientSize.Width, RowH), Theme.TextMuted, Draw.Center);
                return;
            }

            for (int i = 0; i < Rows.Count; i++)
            {
                StrategyTestRow row = Rows[i];
                int top = HeadH + i * RowH;
                Rectangle rr = new Rectangle(4, top + 1, ClientSize.Width - 8, RowH - 2);
                bool selected = i == SelectedIndex;
                bool best = BestScore > 0 && row.Passed == BestScore && row.Total > 0;
                Color fill = selected ? Theme.Selection : (i == _hover ? Color.FromArgb(25, 21, 18) : Color.Transparent);
                if (fill != Color.Transparent) Draw.FillRound(g, rr, 7, fill);
                if (best) Draw.FillRound(g, new Rectangle(rr.X, rr.Y + 8, 3, rr.Height - 16), 2, Theme.Accent);

                Color nameColor = row.Error.Length > 0 ? Theme.Danger : (best || selected ? Theme.Text : Color.FromArgb(226, 216, 205));
                string name = row.StrategyName;
                if (best) name = name + "  ★";
                TextRenderer.DrawText(g, name, best ? Theme.Bold : Theme.Base, new Rectangle(18, top, NameWidth - 24, RowH), nameColor,
                    TextFormatFlags.Left | TextFormatFlags.VerticalCenter | TextFormatFlags.NoPrefix | TextFormatFlags.EndEllipsis);
                if (row.Error.Length > 0)
                {
                    TextRenderer.DrawText(g, "не запустилась", Theme.Small, new Rectangle(18 + NameWidth - 110, top, 104, RowH), Theme.Danger,
                        TextFormatFlags.Right | TextFormatFlags.VerticalCenter | TextFormatFlags.NoPrefix);
                }

                gx = NameWidth;
                foreach (GroupResult gr in row.Groups)
                {
                    Color c = gr.Total == 0 ? Theme.TextMuted : (gr.Ok ? Theme.Success : (gr.Partial ? Theme.Warn : Theme.Danger));
                    TextRenderer.DrawText(g, gr.CellText, Theme.Base, new Rectangle(gx, top, GroupWidth, RowH), c,
                        TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter | TextFormatFlags.NoPrefix);
                    gx += GroupWidth;
                }

                string score = row.Total > 0 ? row.Passed + " / " + row.Total : "—";
                Color sc = row.Total == 0 ? Theme.TextMuted : (row.AllOk ? Theme.Success : (row.Passed > 0 ? Theme.Warn : Theme.Danger));
                TextRenderer.DrawText(g, score, Theme.Bold, new Rectangle(ClientSize.Width - 100, top, 88, RowH), sc,
                    TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter | TextFormatFlags.NoPrefix);
            }
        }
    }

    public sealed class TestPage : PageBase
    {
        private Card _optionsCard;
        private Card _resultCard;
        private Card _officialCard;
        private DarkSelect _reportSelect;
        private DarkList _officialList;
        private Label _officialInfo;
        private List<string> _reportFiles = new List<string>();
        private OfficialTestReport _official;
        private TestGrid _grid;
        private Label _summary;
        private Label _status;
        private LineProgress _progress;
        private RoundButton _btnRun;
        private RoundButton _btnStop;
        private RoundButton _btnApply;
        private RoundButton _btnService;
        private RoundButton _btnSave;

        private List<TestTargetGroup> _groups = new List<TestTargetGroup>();
        private List<string> _enabled = new List<string>();
        private Dictionary<string, RoundButton> _groupButtons = new Dictionary<string, RoundButton>();
        private List<StrategyTestRow> _rows = new List<StrategyTestRow>();
        private int _timeoutMs = 4000;
        private bool _deep;
        private bool _running;
        private volatile bool _cancelled;
        private bool _loadedReport;
        private string _reportPath = "";

        public TestPage(Shell shell) : base(shell) { }

        public override string Key { get { return "test"; } }
        public override string Title { get { return "Тест стратегий"; } }
        public override string Subtitle { get { return "Автоматический прогон стратегий с наглядными результатами"; } }
        public override string Icon { get { return "\uE768"; } }

        private string ReportPath
        {
            get
            {
                if (_reportPath.Length == 0) _reportPath = Path.Combine(AppPaths.DataDir, "test-results.tsv");
                return _reportPath;
            }
        }

        public override void Build()
        {
            BuildOptions();
            BuildResults();
            BuildOfficialCard();
            LoadTargets();
        }

        private void BuildOptions()
        {
            _optionsCard = NewCard("Параметры прогона", 262);
            _optionsCard.Controls.Add(WrapLabel("Тест по очереди запускает стратегии через winws.exe и проверяет доступность целей из utils\\targets.txt. Текущий bypass будет остановлен, после прогона восстановлен.",
                Theme.Small, Theme.TextMuted, 22, 40, DesignWidth - 60));

            _optionsCard.Controls.Add(L("Проверяемые цели:", Theme.Small, Theme.TextDim, 22, 80));
            _optionsCard.Controls.Add(L("Таймаут:", Theme.Small, Theme.TextDim, 22, 146));
            RoundButton t3 = Btn("3 с", ButtonKind.Secondary, 62);
            t3.Location = new Point(90, 140);
            t3.Tag = 3000;
            t3.Click += delegate(object s, EventArgs e) { SetTimeout((int)((RoundButton)s).Tag); };
            _optionsCard.Controls.Add(t3);
            RoundButton t4 = Btn("5 с", ButtonKind.Secondary, 62);
            t4.Location = new Point(158, 140);
            t4.Tag = 5000;
            t4.Click += delegate(object s, EventArgs e) { SetTimeout((int)((RoundButton)s).Tag); };
            _optionsCard.Controls.Add(t4);
            RoundButton t8 = Btn("8 с", ButtonKind.Secondary, 62);
            t8.Location = new Point(226, 140);
            t8.Tag = 8000;
            t8.Click += delegate(object s, EventArgs e) { SetTimeout((int)((RoundButton)s).Tag); };
            _optionsCard.Controls.Add(t8);
            _timeoutButtons = new RoundButton[] { t3, t4, t8 };

            Label lbDeep = L("Глубокая проверка (загрузка 16 КБ)", Theme.Small, Theme.TextDim, 308, 150);
            _optionsCard.Controls.Add(lbDeep);
            _tgDeep = new ToggleSwitch();
            _tgDeep.Location = new Point(lbDeep.Right + 10, 144);
            _tgDeep.CheckedChanged += delegate { _deep = _tgDeep.Checked; };
            _optionsCard.Controls.Add(_tgDeep);

            _btnRun = Btn("Запустить тест", ButtonKind.Primary, 170);
            _btnRun.Location = new Point(22, 182);
            _btnRun.Click += delegate { StartTest(); };
            _optionsCard.Controls.Add(_btnRun);

            _btnStop = Btn("Остановить", ButtonKind.Danger, 120);
            _btnStop.Location = new Point(202, 182);
            _btnStop.Enabled = false;
            _btnStop.Click += delegate { CancelTest(); };
            _optionsCard.Controls.Add(_btnStop);

            RoundButton openTargets = Btn("targets.txt", ButtonKind.Ghost, 120);
            openTargets.Location = new Point(332, 182);
            openTargets.Click += delegate
            {
                if (Shell.Install != null && Shell.Install.IsValid) Native.SelectInExplorer(Path.Combine(Shell.Install.UtilsDir, "targets.txt"));
            };
            _optionsCard.Controls.Add(openTargets);

            _progress = new LineProgress();
            _progress.Location = new Point(22, 222);
            _progress.Width = DesignWidth - 44;
            _progress.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right;
            _optionsCard.Controls.Add(_progress);

            _status = L("", Theme.Small, Theme.TextMuted, 22, 238);
            _optionsCard.Controls.Add(_status);
            View.Stack.Add(_optionsCard);
        }

        private RoundButton[] _timeoutButtons;
        private ToggleSwitch _tgDeep;

        private void BuildResults()
        {
            _resultCard = NewCard("Результаты", 190);
            _summary = L("Прогон ещё не выполнялся.", Theme.Base, Theme.TextDim, 22, 44);
            _resultCard.Controls.Add(_summary);

            _grid = new TestGrid();
            _grid.Location = new Point(20, 76);
            _grid.Width = DesignWidth - 40;
            _grid.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right;
            _grid.SelectedIndexChanged += delegate { UpdateApplyButton(); };
            _resultCard.Controls.Add(_grid);
            _resultCard.Resize += delegate { _grid.Width = _resultCard.ClientSize.Width - 40; };

            _btnApply = Btn("Запустить выбранную", ButtonKind.Primary, 180);
            _btnApply.Enabled = false;
            _btnApply.Click += delegate { ApplySelected(false); };
            _resultCard.Controls.Add(_btnApply);

            _btnService = Btn("Установить как службу", ButtonKind.Secondary, 190);
            _btnService.Enabled = false;
            _btnService.Click += delegate { ApplySelected(true); };
            _resultCard.Controls.Add(_btnService);

            _btnSave = Btn("Сохранить отчёт…", ButtonKind.Ghost, 160);
            _btnSave.Click += delegate { SaveReport(); };
            _resultCard.Controls.Add(_btnSave);

            View.Stack.Add(_resultCard);
            LayoutResultBottom();
        }

        /// <summary>Пересчитывает высоту карточки результатов под число строк.</summary>
        private void LayoutResultBottom()
        {
            _grid.UpdateHeight();
            int buttonsY = _grid.Bottom + 12;
            _btnApply.Location = new Point(22, buttonsY);
            _btnService.Location = new Point(212, buttonsY);
            _btnSave.Location = new Point(412, buttonsY);
            _resultCard.Height = buttonsY + 34 + 16;
            if (_resultCard.Parent != null) View.Stack.PerformLayout();
        }

        private void BuildOfficialCard()
        {
            _officialCard = NewCard("Отчёт официального теста (utils\test zapret.ps1)", 320);
            _officialInfo = L("", Theme.Small, Theme.TextMuted, 22, 40);
            _officialCard.Controls.Add(_officialInfo);

            _reportSelect = new DarkSelect();
            _reportSelect.Location = new Point(22, 64);
            _reportSelect.Width = 420;
            _reportSelect.SelectedIndexChanged += delegate { LoadOfficialReport(); };
            _officialCard.Controls.Add(_reportSelect);

            RoundButton refresh = Btn("Обновить список", ButtonKind.Ghost, 160);
            refresh.Location = new Point(454, 60);
            refresh.Click += delegate { LoadReportList(true); };
            _officialCard.Controls.Add(refresh);

            RoundButton openFolder = Btn("Открыть папку", ButtonKind.Ghost, 140);
            openFolder.Location = new Point(620, 60);
            openFolder.Click += delegate
            {
                if (Shell.Install != null && Shell.Install.IsValid)
                    Native.OpenFolder(Path.Combine(Shell.Install.UtilsDir, "test results"));
            };
            _officialCard.Controls.Add(openFolder);

            _officialList = new DarkList();
            _officialList.Location = new Point(22, 102);
            _officialList.Size = new Size(DesignWidth - 44, 168);
            _officialList.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right;
            _officialCard.Controls.Add(_officialList);

            _officialCard.Controls.Add(L("Нажмите «Запустить официальный тест» на странице «Инструменты», чтобы получить свежий отчёт. Таблица выше — это быстрый прогон средствами менеджера.",
                Theme.Small, Theme.TextMuted, 22, 280));
            View.Stack.Add(_officialCard);
        }

        private string ResultsDir
        {
            get
            {
                return Shell.Install != null && Shell.Install.IsValid
                    ? Path.Combine(Shell.Install.UtilsDir, "test results")
                    : "";
            }
        }

        private void LoadReportList(bool force)
        {
            if (ResultsDir.Length == 0) { _officialCard.Visible = false; return; }
            _reportFiles = OfficialTestReport.ListReports(ResultsDir);
            string keep = _reportSelect.SelectedItem;
            System.Collections.Generic.List<string> names = new System.Collections.Generic.List<string>();
            foreach (string f in _reportFiles) names.Add(Path.GetFileName(f));
            if (names.Count == 0)
            {
                _officialCard.Visible = false;
                return;
            }
            _officialCard.Visible = true;
            _reportSelect.SetItems(names, keep.Length > 0 ? keep : names[0]);
            if (!force) LoadOfficialReport();
            else LoadOfficialReport();
        }

        private void LoadOfficialReport()
        {
            int idx = _reportSelect.SelectedIndex;
            if (idx < 0 || idx >= _reportFiles.Count) return;
            _official = OfficialTestReport.Parse(_reportFiles[idx]);
            _officialInfo.Text = string.Format("{0}  •  {1}  •  стратегий в отчёте: {2}  •  лучшая: {3}",
                Path.GetFileName(_official.FilePath), _official.Created.ToString("dd.MM.yyyy HH:mm"),
                _official.Rows.Count, _official.Best.Length > 0 ? _official.Best : "—");

            _officialList.BeginUpdate();
            _officialList.Items.Clear();
            System.Collections.Generic.List<string> subs = new System.Collections.Generic.List<string>();
            foreach (OfficialTestRow r in _official.Rows)
            {
                bool best = string.Equals(r.Name, _official.Best, StringComparison.OrdinalIgnoreCase);
                _officialList.Items.Add((best ? "★ " : "") + r.Name);
                subs.Add(r.Details);
            }
            _officialList.Subtitles = subs.ToArray();
            _officialList.EndUpdate();
            View.Stack.PerformLayout();
        }
        private void LoadTargets()
        {
            foreach (RoundButton b in _groupButtons.Values) b.Dispose();
            _groupButtons.Clear();
            _enabled.Clear();
            _groups = new List<TestTargetGroup>();
            if (Shell.Install == null || !Shell.Install.IsValid)
            {
                return;
            }
            _groups = TargetsFile.Load(Path.Combine(Shell.Install.UtilsDir, "targets.txt"));
            int x = 22;
            foreach (TestTargetGroup g in _groups)
            {
                RoundButton b = Btn(g.Name, ButtonKind.Secondary, 0);
                b.Fit(10);
                b.Height = 26;
                b.Location = new Point(x, 104);
                string name = g.Name;
                b.Click += delegate { ToggleGroup(name); };
                _optionsCard.Controls.Add(b);
                _groupButtons[name] = b;
                x += b.Width + 8;
                if (IsDefaultGroup(g.Name)) _enabled.Add(g.Name);
            }
            if (_enabled.Count == 0)
            {
                foreach (TestTargetGroup g in _groups) _enabled.Add(g.Name);
            }
            RefreshGroupButtons();
        }

        private static bool IsDefaultGroup(string name)
        {
            string n = name.ToLowerInvariant();
            return n.Contains("discord") || n.Contains("youtube") || n.Contains("google") || n.Contains("cloudflare");
        }

        private void ToggleGroup(string name)
        {
            if (_enabled.Contains(name)) _enabled.Remove(name);
            else _enabled.Add(name);
            RefreshGroupButtons();
        }

        private void RefreshGroupButtons()
        {
            foreach (KeyValuePair<string, RoundButton> kv in _groupButtons)
            {
                bool on = _enabled.Contains(kv.Key);
                kv.Value.Kind = on ? ButtonKind.Primary : ButtonKind.Secondary;
                kv.Value.Invalidate();
            }
        }

        private void SetTimeout(int ms)
        {
            _timeoutMs = ms;
            if (_timeoutButtons == null) return;
            foreach (RoundButton b in _timeoutButtons)
            {
                int v = Convert.ToInt32(b.Tag);
                b.Kind = v == _timeoutMs ? ButtonKind.Primary : ButtonKind.Secondary;
                b.Invalidate();
            }
        }

        private List<TestTargetGroup> SelectedGroups()
        {
            List<TestTargetGroup> list = new List<TestTargetGroup>();
            foreach (TestTargetGroup g in _groups)
            {
                if (_enabled.Contains(g.Name)) list.Add(g);
            }
            return list;
        }

        private void StartTest()
        {
            if (_running) return;
            if (!Shell.Install.IsValid) { Shell.Notify("Папка zapret не найдена.", LogLevel.Error); return; }
            if (!Shell.RequireAdmin("Тест стратегий")) return;
            List<TestTargetGroup> groups = SelectedGroups();
            if (groups.Count == 0) { Shell.Notify("Выберите хотя бы одну группу целей.", LogLevel.Warn); return; }
            if (Shell.Install.Strategies.Count == 0) { Shell.Notify("Стратегии не найдены.", LogLevel.Error); return; }

            int targets = 0;
            foreach (TestTargetGroup g in groups) targets += g.Targets.Count;
            int strategies = Shell.Install.Strategies.Count;
            int minutes = (int)Math.Ceiling(strategies * (1.7 + Math.Min(targets, 6) * 0.6) / 60.0);
            if (!Dlg.Confirm(this.View.FindForm(), "Запустить тест?",
                string.Format("Будут проверены {0} стратегий по {1} целям. Bypass будет остановлен, служба zapret (если есть) — временно выключена, после прогона всё вернётся на место.\n\nОриентировочно: до {2} мин.",
                    strategies, targets, Math.Max(1, minutes)), "Запустить"))
                return;

            _running = true;
            _cancelled = false;
            _rows = new List<StrategyTestRow>();
            _grid.Rows = _rows;
            _grid.Groups = GroupNames(groups);
            _grid.BestScore = 0;
            _grid.UpdateHeight();
            LayoutResultBottom();
            _summary.Text = "Прогон…";
            _summary.ForeColor = Theme.TextDim;
            _btnRun.Enabled = false;
            _btnStop.Enabled = true;
            _progress.Value = 0;
            SetTimeoutButtonsEnabled(false);
            _optionsCard.Invalidate();
            Shell.Form.SetStatusHint("Прогон стратегий…");

            TesterOptions opt = new TesterOptions();
            opt.TimeoutMs = _timeoutMs;
            opt.DeepCheck = _deep;
            opt.RestoreAfter = true;
            StrategyTester tester = new StrategyTester(Shell.Install, Shell.Ops);
            List<StrategyDef> defs = new List<StrategyDef>(Shell.Install.Strategies);
            DateTime started = DateTime.Now;

            Shell.RunBackground(delegate()
            {
                List<StrategyTestRow> result = tester.Run(groups, defs, opt,
                    delegate(int done, int total, string name)
                    {
                        Shell.Ui(delegate()
                        {
                            _progress.Value = (int)((done - 1) * 100.0 / total);
                            _status.Text = string.Format("Стратегия {0} из {1}: {2}", done, total, name);
                        });
                    },
                    delegate(StrategyTestRow row)
                    {
                        Shell.Ui(delegate()
                        {
                            _rows.Add(row);
                            _grid.Rows = _rows;
                            _grid.BestScore = BestScore(_rows);
                            _grid.UpdateHeight();
                            LayoutResultBottom();
                            _grid.Invalidate();
                            UpdateSummary();
                        });
                    },
                    delegate() { return _cancelled; });

                Shell.Ui(delegate()
                {
                    _running = false;
                    _btnRun.Enabled = true;
                    _btnStop.Enabled = false;
                    SetTimeoutButtonsEnabled(true);
                    _progress.Value = 100;
                    _status.Text = _cancelled ? "Остановлено пользователем." : "Готово.";
                    Shell.Form.SetStatusHint("");
                    StrategyTester.SaveCsv(ReportPath, _rows, _grid.Groups, opt);
                    UpdateSummary();
                    _loadedReport = false;
                    Log.Ok(string.Format("Прогон стратегий завершён за {0}: проверено {1}, лучший результат {2}/{3}",
                        FormatDuration(DateTime.Now - started), _rows.Count, BestScore(_rows), rowTotal()));
                    Shell.Notify("Тест завершён: " + _summary.Text, LogLevel.Ok);
                });
            }, null);
        }

        private int rowTotal()
        {
            int t = 0;
            foreach (StrategyTestRow r in _rows) if (r.Total > t) t = r.Total;
            return t;
        }

        private void SetTimeoutButtonsEnabled(bool enabled)
        {
            if (_timeoutButtons != null) foreach (RoundButton b in _timeoutButtons) b.Enabled = enabled;
            _tgDeep.Enabled = enabled;
            foreach (RoundButton b in _groupButtons.Values) b.Enabled = enabled;
        }

        private void CancelTest()
        {
            _cancelled = true;
            _btnStop.Enabled = false;
            _status.Text = "Останавливаю…";
        }

        private static List<string> GroupNames(List<TestTargetGroup> groups)
        {
            List<string> names = new List<string>();
            foreach (TestTargetGroup g in groups) names.Add(g.Name);
            return names;
        }

        private static int BestScore(List<StrategyTestRow> rows)
        {
            int best = 0;
            foreach (StrategyTestRow r in rows)
            {
                if (r.Error.Length == 0 && r.Passed > best) best = r.Passed;
            }
            return best;
        }

        private void UpdateSummary()
        {
            if (_rows.Count == 0)
            {
                _summary.Text = "Прогон ещё не выполнялся.";
                _summary.ForeColor = Theme.TextDim;
                return;
            }
            StrategyTestRow best = null;
            foreach (StrategyTestRow r in _rows)
            {
                if (r.Error.Length > 0) continue;
                if (best == null || r.Passed > best.Passed) best = r;
            }
            int total = rowTotal();
            if (best == null)
            {
                _summary.Text = "Ни одна стратегия не запустилась — проверьте права администратора и драйвер WinDivert.";
                _summary.ForeColor = Theme.Danger;
            }
            else if (best.Passed == 0)
            {
                _summary.Text = string.Format("Проверено {0} стратегий, ни одна не дала доступа к целям (0 из {1}). Возможно, проблема на стороне сети.", _rows.Count, total);
                _summary.ForeColor = Theme.Warn;
            }
            else
            {
                _summary.Text = string.Format("Лучшая: {0} — {1} из {2}. Проверено стратегий: {3}.", best.StrategyName, best.Passed, total, _rows.Count);
                _summary.ForeColor = best.AllOk ? Theme.Success : Theme.Warn;
            }
            UpdateApplyButton();
        }

        private void UpdateApplyButton()
        {
            bool has = _grid != null && _grid.SelectedIndex >= 0 && _grid.SelectedIndex < _rows.Count;
            _btnApply.Enabled = has && !_running;
            _btnService.Enabled = has && !_running;
        }

        private void ApplySelected(bool asService)
        {
            if (_grid.SelectedIndex < 0 || _grid.SelectedIndex >= _rows.Count) return;
            StrategyTestRow row = _rows[_grid.SelectedIndex];
            StrategyDef def = Shell.Install.FindStrategy(row.StrategyName);
            if (def == null) { Shell.Notify("Стратегия не найдена в папке zapret.", LogLevel.Error); return; }
            if (!Shell.RequireAdmin(asService ? "Установка службы" : "Запуск")) return;
            if (asService)
            {
                if (!Dlg.Confirm(this.View.FindForm(), "Установить службу?",
                    string.Format("Стратегия «{0}» будет установлена как служба zapret (автозапуск).", def.Name), "Установить")) return;
                bool ok;
                string msg = Shell.Ops.InstallService(def, out ok);
                Shell.Notify(msg, ok ? LogLevel.Ok : LogLevel.Error);
            }
            else
            {
                if (Shell.State != null && Shell.State.ServiceInstalled && Shell.State.ServiceStatus == "работает")
                {
                    if (!Dlg.Confirm(this.View.FindForm(), "Остановить службу?",
                        "Сейчас bypass работает как служба. Остановить службу и запустить выбранную стратегию вручную?", "Запустить")) return;
                    Shell.Ops.Stop();
                }
                ZapretOps.StartResult r2 = Shell.Ops.Start(def);
                Shell.Notify(r2.Message, r2.Ok ? LogLevel.Ok : LogLevel.Error);
            }
            Shell.Config.Set("LastStrategy", def.Name);
            Shell.Config.Save();
            Shell.RefreshState(false);
        }

        private void SaveReport()
        {
            if (_rows.Count == 0) { Shell.Notify("Сначала выполните прогон.", LogLevel.Warn); return; }
            SaveFileDialog dlg = new SaveFileDialog();
            dlg.Title = "Сохранить отчёт теста";
            dlg.Filter = "Таблица (*.tsv)|*.tsv|Все файлы (*.*)|*.*";
            dlg.FileName = "zapret-strategy-test-" + DateTime.Now.ToString("yyyyMMdd-HHmm") + ".tsv";
            if (dlg.ShowDialog() != DialogResult.OK) return;
            TesterOptions opt = new TesterOptions();
            opt.TimeoutMs = _timeoutMs;
            opt.DeepCheck = _deep;
            StrategyTester.SaveCsv(dlg.FileName, _rows, _grid.Groups, opt);
            Shell.Notify("Отчёт сохранён: " + dlg.FileName, LogLevel.Ok);
        }

        private static string FormatDuration(TimeSpan t)
        {
            if (t.TotalSeconds < 60) return string.Format("{0} с", (int)t.TotalSeconds);
            return string.Format("{0} мин {1} с", (int)t.TotalMinutes, t.Seconds);
        }

        public override void OnShow()
        {
            if (_groups.Count == 0) LoadTargets();
            if (_reportFiles.Count == 0) LoadReportList(false);
            if (_rows.Count == 0 && !_loadedReport) TryLoadReport();
            SetTimeout(_timeoutMs);
            RefreshState();
        }

        private void TryLoadReport()
        {
            try
            {
                if (!File.Exists(ReportPath)) return;
                StrategyTester.LoadedReport rep = StrategyTester.LoadCsv(ReportPath);
                if (rep.Rows.Count == 0) return;
                _rows = rep.Rows;
                _grid.Rows = _rows;
                _grid.Groups = rep.Groups;
                _grid.BestScore = BestScore(_rows);
                _grid.UpdateHeight();
                LayoutResultBottom();
                _grid.Invalidate();
                _loadedReport = true;
                UpdateSummary();
                _status.Text = "Показаны результаты предыдущего прогона (" + File.GetLastWriteTime(ReportPath).ToString("dd.MM.yyyy HH:mm") + ").";
            }
            catch (Exception ex) { Log.Warn("Не удалось загрузить прошлый отчёт: " + ex.Message); }
        }

        public override void RefreshState()
        {
            if (_running) return;
            if (!Shell.Install.IsValid)
            {
                _status.Text = "Папка zapret не найдена.";
            }
            else if (_status.Text.Length == 0)
            {
                _status.Text = string.Format("Стратегий: {0} · групп целей: {1} · выбрано: {2}", Shell.Install.StrategyCount, _groups.Count, _enabled.Count);
            }
        }
    }
}
