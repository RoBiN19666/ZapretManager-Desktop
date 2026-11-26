using System;
using System.Drawing;
using System.Windows.Forms;

namespace ZapretManager
{
    public sealed class StrategiesPage : PageBase
    {
        private Card _listCard;
        private Card _cmdCard;
        private StrategyList _list;
        private Label _listDesc;
        private Label _preview;
        private Label _hint;
        private Label _cmdTitle;
        private RoundButton _btnStart;
        private RoundButton _btnService;
        private RoundButton _btnOpen;
        private RoundButton _btnCopy;

        public StrategiesPage(Shell shell) : base(shell) { }

        public override string Key { get { return "strategies"; } }
        public override string Title { get { return "Стратегии"; } }
        public override string Subtitle { get { return "Выбор и запуск стратегий обхода (файлы general*.bat)"; } }
        public override string Icon { get { return "\uE945"; } }

        public override void Build()
        {
            _listCard = NewCard("Доступные стратегии", 504);
            _listDesc = L("", Theme.Small, Theme.TextMuted, 22, 40);
            _listDesc.AutoSize = false;
            _listDesc.AutoEllipsis = false;
            _listDesc.Size = new Size(DesignWidth - 44, Theme.Px(22));
            _listDesc.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right;
            _listCard.Controls.Add(_listDesc);

            _list = new StrategyList();
            _list.Location = new Point(20, 68);
            _list.Size = new Size(DesignWidth - 40, 354);
            _list.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right | AnchorStyles.Bottom;
            _list.SelectedIndexChanged += delegate { UpdatePreview(); UpdateListDescription(); };
            _list.HoverChanged += delegate { UpdateListDescription(); };
            _list.ItemActivated += delegate { StartSelected(true); };
            _listCard.Controls.Add(_list);
            _listCard.Resize += delegate
            {
                _listDesc.Width = Math.Max(100, _listCard.ClientSize.Width - Theme.Px(44));
                _list.Size = new Size(_listCard.ClientSize.Width - 40,
                    Math.Max(80, _listCard.ClientSize.Height - Theme.Px(68) - Theme.Px(82)));
            };

            _hint = L("", Theme.Small, Theme.Warn, 22, 430);
            _hint.Anchor = AnchorStyles.Left | AnchorStyles.Bottom;
            _listCard.Controls.Add(_hint);

            int y = 456;
            _btnStart = Btn("Запустить", ButtonKind.Primary, 128);
            _btnStart.Location = new Point(20, y);
            _btnStart.Anchor = AnchorStyles.Left | AnchorStyles.Bottom;
            _btnStart.Click += delegate { StartSelected(false); };
            _listCard.Controls.Add(_btnStart);

            _btnService = Btn("Установить как службу", ButtonKind.Secondary, 190);
            _btnService.Location = new Point(156, y);
            _btnService.Anchor = AnchorStyles.Left | AnchorStyles.Bottom;
            _btnService.Click += delegate { InstallServiceSelected(); };
            _listCard.Controls.Add(_btnService);

            _btnOpen = Btn("Открыть .bat", ButtonKind.Ghost, 116);
            _btnOpen.Location = new Point(354, y);
            _btnOpen.Anchor = AnchorStyles.Left | AnchorStyles.Bottom;
            _btnOpen.Click += delegate { OpenSelected(); };
            _listCard.Controls.Add(_btnOpen);

            _btnCopy = Btn("Копировать команду", ButtonKind.Ghost, 190);
            _btnCopy.Location = new Point(478, y);
            _btnCopy.Anchor = AnchorStyles.Left | AnchorStyles.Bottom;
            _btnCopy.Click += delegate { CopyCommand(); };
            _listCard.Controls.Add(_btnCopy);

            View.Stack.Add(_listCard);

            _cmdCard = NewCard("Команда запуска (winws.exe) — полный текст по кнопке «Копировать команду»", 200);
            _cmdTitle = L("", Theme.Small, Theme.TextMuted, 22, 40);
            _cmdCard.Controls.Add(_cmdTitle);
            _preview = new Label();
            _preview.BackColor = Theme.Surface;
            _preview.ForeColor = Theme.TextDim;
            _preview.Font = Theme.Mono;
            _preview.AutoSize = false;
            _preview.Location = new Point(20, 62);
            _preview.Size = new Size(DesignWidth - 40, 120);
            _preview.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right | AnchorStyles.Bottom;
            _cmdCard.Controls.Add(_preview);
            _cmdCard.Resize += delegate
            {
                _preview.Size = new Size(_cmdCard.ClientSize.Width - 40, Math.Max(40, _cmdCard.ClientSize.Height - 80));
            };
            View.Stack.Add(_cmdCard);
        }

        private StrategyDef SelectedStrategy()
        {
            if (_list == null) return null;
            int i = _list.SelectedIndex;
            if (i < 0 || i >= _list.Items2.Count)
            {
                if (_list.Items2.Count > 0) { i = 0; _list.SelectedIndex = 0; }
                else return null;
            }
            return _list.Items2[i];
        }

        public override void OnShow()
        {
            if (Shell.Install != null && Shell.Install.IsValid && _list.Items2.Count == 0)
            {
                _list.SetItems(Shell.Install.Strategies);
                if (Shell.State != null && Shell.State.MatchedStrategy.Length > 0)
                {
                    StrategyDef cur = Shell.Install.FindStrategy(Shell.State.MatchedStrategy);
                    if (cur != null) _list.SelectedIndex = cur.Order;
                }
                if (_list.SelectedIndex < 0 && _list.Items2.Count > 0) _list.SelectedIndex = 0;
            }
            UpdatePreview();
            UpdateListDescription();
        }

        public override void RefreshState()
        {
            if (Shell.Install == null || !Shell.Install.IsValid) return;
            if (_list.Items2.Count != Shell.Install.Strategies.Count) _list.SetItems(Shell.Install.Strategies);
            string active = Shell.State != null ? Shell.State.MatchedStrategy : "";
            if (_list.ActiveName != active)
            {
                _list.ActiveName = active;
                _list.Invalidate();
            }
            UpdatePreview();
            UpdateListDescription();
        }

        private bool _descBusy;

        private void UpdateListDescription()
        {
            if (_listDesc == null || _list == null || _descBusy) return;
            int i = _list.HoverIndex;
            if (i < 0) i = _list.SelectedIndex;
            string text = (i >= 0 && i < _list.Items2.Count) ? _list.Items2[i].Description : "";
            if (_listDesc.Text == text) return;
            _descBusy = true;
            try { _listDesc.Text = text; }
            finally { _descBusy = false; }
        }

        private void UpdatePreview()
        {
            StrategyDef s = SelectedStrategy();
            if (s == null)
            {
                _preview.Text = "";
                _cmdTitle.Text = "";
                return;
            }
            if (Shell.Install == null || !Shell.Install.IsValid) return;
            GameFilterState gf = GameFilterState.Load(Shell.Install.Root);
            string err;
            string[] argv = BatParser.GetArgv(Shell.Install, s, gf, out err);
            if (argv == null)
            {
                _preview.Text = "Ошибка разбора: " + err;
                _cmdTitle.Text = s.Name;
            }
            else
            {
                _preview.Text = "\"" + Shell.Install.WinwsExe + "\" " + BatParser.BuildCommandLine(argv);
                _cmdTitle.Text = string.Format("{0}  •  аргументов: {1}  •  {2}", s.Name, argv.Length, s.Description);
            }
            _hint.Text = s.Warning ? "Внимание: авторы помечают эту стратегию как «NOT RECOMMENDED»." : "";
        }

        private void StartSelected(bool doubleClick)
        {
            StrategyDef s = SelectedStrategy();
            if (s == null) return;
            if (!Shell.RequireAdmin("Запуск стратегии")) return;
            RuntimeState st = Shell.State;
            bool stopService = st != null && st.ServiceInstalled && st.ServiceStatus == "работает";
            if (stopService)
            {
                if (!Dlg.Confirm(this.View.FindForm(), "Служба запущена",
                    "Стратегия сейчас работает как служба zapret. Остановить службу и запустить выбранную стратегию вручную?", "Остановить службу и запустить"))
                    return;
            }
            ZapretOps.StartResult r = null;
            Shell.RunOperation(delegate()
            {
                if (stopService) Shell.Ops.Stop();
                r = Shell.Ops.Start(s);
                return r.Message;
            }, delegate(string message)
            {
                Shell.Config.Set("LastStrategy", s.Name);
                Shell.Config.Save();
                Shell.Notify(message, r != null && r.Ok ? LogLevel.Ok : LogLevel.Error);
                Shell.RefreshState(false);
            });
        }

        private void InstallServiceSelected()
        {
            StrategyDef s = SelectedStrategy();
            if (s == null) return;
            if (!Shell.RequireAdmin("Установка службы")) return;
            if (!Dlg.Confirm(this.View.FindForm(), "Установить службу?",
                string.Format("Стратегия «{0}» будет установлена как служба Windows «zapret» и будет запускаться автоматически при загрузке. Текущий запущенный bypass будет остановлен.", s.Name),
                "Установить")) return;

            bool ok = false;
            string msg = "";
            Shell.RunOperation(delegate() { msg = Shell.Ops.InstallService(s, out ok); return msg; }, delegate(string message)
            {
                Shell.Config.Set("LastStrategy", s.Name);
                Shell.Config.Save();
                Shell.Notify(message, ok ? LogLevel.Ok : LogLevel.Error);
                Shell.RefreshState(false);
                if (!ok) Dlg.Info(this.View.FindForm(), "Не удалось установить службу", message);
            });
        }

        private void OpenSelected()
        {
            StrategyDef s = SelectedStrategy();
            if (s == null) return;
            try
            {
                // .bat через ShellExecute был бы ЗАПУСКОМ стратегии, а не просмотром.
                using (System.Diagnostics.Process p = System.Diagnostics.Process.Start(
                    new System.Diagnostics.ProcessStartInfo("notepad.exe", "\"" + s.File + "\"") { UseShellExecute = true })) { }
            }
            catch (Exception ex) { Shell.Notify("Не удалось открыть файл: " + ex.Message, LogLevel.Error); }
        }

        private void CopyCommand()
        {
            try
            {
                if (_preview.Text.Length > 0)
                {
                    Clipboard.SetText(_preview.Text);
                    Shell.Notify("Команда скопирована в буфер обмена.", LogLevel.Ok);
                }
            }
            catch (Exception ex) { Shell.Notify("Не удалось скопировать: " + ex.Message, LogLevel.Error); }
        }
    }
}
