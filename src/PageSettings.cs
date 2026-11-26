using System;
using System.Drawing;
using System.IO;
using System.Windows.Forms;

namespace ZapretManager
{
    public sealed class SettingsPage : PageBase
    {
        private Card _gameCard;
        private Card _ipsetCard;
        private Card _fakeCard;
        private Card _miscCard;
        private Card _startupCard;
        private ToggleSwitch _tgAutoStart;
        private ToggleSwitch _tgCloseToTray;
        private ToggleSwitch _tgAutoZapret;
        private ToggleSwitch _tgAutoTg;
        private ToggleSwitch _tgMinimize;
        private DarkSelect _themeSelect;
        private Label _autoStatus;

        private RoundButton[] _modeButtons = new RoundButton[4];
        private string _mode = "disabled";
        private InputBox _tcpBox;
        private InputBox _udpBox;

        private RoundButton[] _ipsetButtons = new RoundButton[3];
        private string _ipsetMode = "loaded";
        private Label _ipsetInfo;
        private LineProgress _ipsetProgress;
        private RoundButton _btnIpsetUpdate;

        private DarkSelect _discordFake;
        private DarkSelect _gameFake;
        private Label _fakeInfo;
        private ToggleSwitch _tgManagerUpdate;
        private bool _suppress;

        public SettingsPage(Shell shell) : base(shell) { }

        public override string Key { get { return "settings"; } }
        public override string Title { get { return "Настройки"; } }
        public override string Subtitle { get { return "Игровой фильтр, IP-список, фейк-пакеты и прочие параметры"; } }
        public override string Icon { get { return "\uE713"; } }

        public override void Build()
        {
            BuildGameCard();
            BuildIpsetCard();
            BuildFakeCard();
            BuildMiscCard();
            BuildStartupCard();
        }

        private void BuildGameCard()
        {
            _gameCard = NewCard("Игровой фильтр", 190);
            _gameCard.Controls.Add(WrapLabel("Фильтр расширяет обход на игровые порты. Включайте, если игры или голосовой чат работают плохо.",
                Theme.Small, Theme.TextMuted, 22, 40, DesignWidth - 60));
            string[] names = new string[] { "Выключен", "TCP и UDP", "Только TCP", "Только UDP" };
            int x = 22;
            for (int i = 0; i < names.Length; i++)
            {
                RoundButton b = Btn(names[i], ButtonKind.Secondary, 118);
                b.Location = new Point(x, 78);
                b.Tag = i;
                b.Click += delegate(object s, EventArgs e)
                {
                    RoundButton rb = (RoundButton)s;
                    _mode = new string[] { "disabled", "all", "tcp", "udp" }[(int)rb.Tag];
                    ApplyModes();
                };
                _modeButtons[i] = b;
                _gameCard.Controls.Add(b);
                x += 126;
            }

            _gameCard.Controls.Add(L("TCP-порты", Theme.Small, Theme.TextDim, 22, 130));
            _tcpBox = new InputBox();
            _tcpBox.Location = new Point(110, 122);
            _tcpBox.Width = 210;
            _gameCard.Controls.Add(_tcpBox);

            _gameCard.Controls.Add(L("UDP-порты", Theme.Small, Theme.TextDim, 356, 130));
            _udpBox = new InputBox();
            _udpBox.Location = new Point(444, 122);
            _udpBox.Width = 210;
            _gameCard.Controls.Add(_udpBox);

            RoundButton apply = Btn("Применить", ButtonKind.Primary, 120);
            apply.Anchor = AnchorStyles.Top | AnchorStyles.Right;
            apply.Location = new Point(DesignWidth - 22 - 120, 122);
            apply.Click += delegate { ApplyGameFilter(); };
            _gameCard.Controls.Add(apply);
            View.Stack.Add(_gameCard);
        }

        private void BuildIpsetCard()
        {
            _ipsetCard = NewCard("IP-фильтр (IPSet)", 214);
            _ipsetCard.Controls.Add(WrapLabel("Определяет, к каким адресам применяется обход: только к доменам из списков или ко всем IP из полного списка.",
                Theme.Small, Theme.TextMuted, 22, 40, DesignWidth - 60));

            string[] names = new string[] { "Полный список", "Любые IP", "Без фильтра" };
            string[] tags = new string[] { "loaded", "any", "none" };
            int x = 22;
            for (int i = 0; i < names.Length; i++)
            {
                RoundButton b = Btn(names[i], ButtonKind.Secondary, 136);
                b.Location = new Point(x, 78);
                b.Tag = tags[i];
                b.Click += delegate(object s, EventArgs e)
                {
                    RoundButton rb = (RoundButton)s;
                    ApplyIpsetMode(Convert.ToString(rb.Tag));
                };
                _ipsetButtons[i] = b;
                _ipsetCard.Controls.Add(b);
                x += 144;
            }

            _ipsetInfo = L("", Theme.Small, Theme.TextMuted, 22, 126);
            _ipsetCard.Controls.Add(_ipsetInfo);

            _btnIpsetUpdate = Btn("Обновить IP-лист", ButtonKind.Secondary, 160);
            _btnIpsetUpdate.Location = new Point(22, 154);
            _btnIpsetUpdate.Click += delegate { UpdateIpset(); };
            _ipsetCard.Controls.Add(_btnIpsetUpdate);

            _ipsetProgress = new LineProgress();
            _ipsetProgress.Location = new Point(196, 166);
            _ipsetProgress.Width = DesignWidth - 220;
            _ipsetProgress.Anchor = AnchorStyles.Left | AnchorStyles.Right | AnchorStyles.Top;
            _ipsetCard.Controls.Add(_ipsetProgress);
            View.Stack.Add(_ipsetCard);
        }

        private void BuildFakeCard()
        {
            _fakeCard = NewCard("Активные фейк-пакеты", 232);
            _fakeCard.Controls.Add(WrapLabel("Подмена «фейковых» пакетов для Discord и игрового фильтра. Если голосовой чат не работает — попробуйте другой файл.",
                Theme.Small, Theme.TextMuted, 22, 40, DesignWidth - 60));

            _fakeCard.Controls.Add(L("Discord (UDP)", Theme.Small, Theme.TextDim, 22, 82));
            _discordFake = new DarkSelect();
            _discordFake.Location = new Point(22, 102);
            _discordFake.Width = 420;
            _fakeCard.Controls.Add(_discordFake);
            RoundButton b1 = Btn("Заменить", ButtonKind.Secondary, 110);
            b1.Anchor = AnchorStyles.Top | AnchorStyles.Right;
            b1.Location = new Point(DesignWidth - 22 - 110, 100);
            b1.Click += delegate { ReplaceFake("ACTIVE_DISCORD_UDP.bin", _discordFake); };
            _fakeCard.Controls.Add(b1);

            _fakeCard.Controls.Add(L("Игровой фильтр (UDP)", Theme.Small, Theme.TextDim, 22, 142));
            _gameFake = new DarkSelect();
            _gameFake.Location = new Point(22, 162);
            _gameFake.Width = 420;
            _fakeCard.Controls.Add(_gameFake);
            RoundButton b2 = Btn("Заменить", ButtonKind.Secondary, 110);
            b2.Anchor = AnchorStyles.Top | AnchorStyles.Right;
            b2.Location = new Point(DesignWidth - 22 - 110, 160);
            b2.Click += delegate { ReplaceFake("ACTIVE_GAME_UDP.bin", _gameFake); };
            _fakeCard.Controls.Add(b2);

            _fakeInfo = L("", Theme.Small, Theme.TextMuted, 22, 198);
            _fakeCard.Controls.Add(_fakeInfo);
            View.Stack.Add(_fakeCard);
        }

        private void BuildStartupCard()
        {
            _startupCard = NewCard("Запуск и автозапуск", 376);

            _tgAutoStart = AddToggleRow(_startupCard, 40, "Запускать Zapret Manager при входе в Windows",
                "Создаётся задача планировщика с наивысшими правами — запуск без запроса UAC", false, delegate
                {
                    if (_suppress) return;
                    if (_tgAutoStart.Checked)
                    {
                        string msg = Autostart.Enable(Application.ExecutablePath, Shell.Config.GetBool("StartMinimized", true));
                        Shell.Notify(msg, msg.StartsWith("Не удалось") ? LogLevel.Error : LogLevel.Ok);
                        if (msg.StartsWith("Не удалось")) _tgAutoStart.Checked = false;
                    }
                    else
                    {
                        string msg = Autostart.Disable();
                        Shell.Notify(msg, LogLevel.Ok);
                    }
                    RefreshStartupStatus();
                });

            _tgCloseToTray = AddToggleRow(_startupCard, 92, "При закрытии окна сворачиваться в трей",
                "Приложение продолжает работать в фоне, bypass и прокси не останавливаются", Shell.Config.GetBool("CloseToTray", true), delegate
                {
                    if (_suppress) return;
                    Shell.Config.SetBool("CloseToTray", _tgCloseToTray.Checked);
                    Shell.Config.Save();
                });

            _tgAutoZapret = AddToggleRow(_startupCard, 144, "При автозапуске поднимать zapret",
                "Запускать выбранную стратегию (или службу), если bypass ещё не работает", Shell.Config.GetBool("AutostartZapret", true), delegate
                {
                    if (_suppress) return;
                    Shell.Config.SetBool("AutostartZapret", _tgAutoZapret.Checked);
                    Shell.Config.Save();
                });

            _tgAutoTg = AddToggleRow(_startupCard, 196, "При автозапуске запускать TG WS Proxy",
                "Прокси Telegram стартует вместе с менеджером", Shell.Config.GetBool("AutostartTgProxy", true), delegate
                {
                    if (_suppress) return;
                    Shell.Config.SetBool("AutostartTgProxy", _tgAutoTg.Checked);
                    Shell.Config.Save();
                });

            _tgMinimize = AddToggleRow(_startupCard, 248, "Сворачивать в трей по кнопке «Свернуть»",
                "Окно прячется в трей, а не висит в панели задач — приложение и сервисы продолжают работать", Shell.Config.GetBool("MinimizeToTray", true), delegate
                {
                    if (_suppress) return;
                    Shell.Config.SetBool("MinimizeToTray", _tgMinimize.Checked);
                    Shell.Config.Save();
                });

            _autoStatus = L("", Theme.Small, Theme.TextMuted, 22, 304);
            _autoStatus.AutoSize = false;
            _autoStatus.Width = 450;
            _autoStatus.Height = 16;
            _startupCard.Controls.Add(_autoStatus);

            RoundButton openTask = Btn("Планировщик заданий", ButtonKind.Ghost, 190);
            openTask.Location = new Point(DesignWidth - 22 - 190, 296);
            openTask.Click += delegate { Autostart.OpenTaskScheduler(); };
            _startupCard.Controls.Add(openTask);

            RoundButton wizard = Btn("Мастер установки", ButtonKind.Ghost, 170);
            wizard.Location = new Point(DesignWidth - 22 - 190 - 180, 296);
            wizard.Click += delegate
            {
                using (SetupWizard w = new SetupWizard(Shell)) { w.ShowDialog(this.View.FindForm()); }
                Shell.RefreshState(true);
                RefreshState();
            };
            _startupCard.Controls.Add(wizard);


            View.Stack.Add(_startupCard);
        }

        private void RefreshStartupStatus()
        {
            bool enabled = Autostart.IsEnabled;
            _suppress = true;
            try { _tgAutoStart.Checked = enabled; }
            finally { _suppress = false; }
            _autoStatus.Text = enabled ? "Автозапуск включён" : "Автозапуск выключен";
            _autoStatus.ForeColor = enabled ? Theme.Success : Theme.TextMuted;
        }
        private static string ThemeChoice(string mode)
        {
            if (string.Equals(mode, "dark", StringComparison.OrdinalIgnoreCase)) return "Тёмная";
            if (string.Equals(mode, "light", StringComparison.OrdinalIgnoreCase)) return "Светлая";
            return "Как в Windows";
        }

        private void BuildMiscCard()
        {
            _miscCard = NewCard("Прочее", 210);

            _miscCard.Controls.Add(L("Оформление", Theme.Small, Theme.TextDim, 22, 38));
            _themeSelect = new DarkSelect();
            _themeSelect.Location = new Point(150, 36);
            _themeSelect.Width = 220;
            _themeSelect.SetItems(new System.Collections.Generic.List<string>(new string[] { "Как в Windows", "Тёмная", "Светлая" }), ThemeChoice(Shell.Config.Get("Theme", "auto")));
            _themeSelect.SelectedIndexChanged += delegate
            {
                if (_suppress) return;
                string mode = _themeSelect.SelectedIndex == 1 ? "dark" : (_themeSelect.SelectedIndex == 2 ? "light" : "auto");
                Shell.Form.ApplyTheme(mode);
                Shell.Notify("Оформление: " + _themeSelect.SelectedItem.ToLowerInvariant(), LogLevel.Ok);
            };
            _miscCard.Controls.Add(_themeSelect);
            _tgManagerUpdate = AddToggleRow(_miscCard, 84, "Проверять обновления при запуске менеджера",
                "При старте и далее раз в 6 часов проверять GitHub (zapret, TG WS Proxy, менеджер)", Shell.Config.GetBool("CheckUpdatesOnStartup", true), delegate
                {
                    if (_suppress) return;
                    Shell.Config.SetBool("CheckUpdatesOnStartup", _tgManagerUpdate.Checked);
                    Shell.Config.Save();
                    Shell.Notify(_tgManagerUpdate.Checked ? "Автопроверка обновлений менеджера включена." : "Автопроверка обновлений менеджера выключена.", LogLevel.Ok);
                });

            RoundButton tcp = Btn("Включить TCP timestamps", ButtonKind.Secondary, 210);
            tcp.Location = new Point(22, 148);
            tcp.Click += delegate
            {
                if (!Shell.RequireAdmin("TCP timestamps")) return;
                bool ok = Shell.Ops.EnableTcpTimestamps();
                Shell.Notify(ok ? "TCP timestamps включены." : "Не удалось включить TCP timestamps.", ok ? LogLevel.Ok : LogLevel.Error);
            };
            _miscCard.Controls.Add(tcp);

            RoundButton dc = Btn("Очистить кэш Discord", ButtonKind.Secondary, 190);
            dc.Location = new Point(244, 148);
            dc.Click += delegate
            {
                if (!Dlg.Confirm(this.View.FindForm(), "Очистить кэш Discord?", "Discord будет закрыт, а папки Cache, Code Cache и GPUCache удалены.", "Очистить")) return;
                string msg = Shell.Ops.ClearDiscordCache();
                Shell.Notify(msg, LogLevel.Ok);
            };
            _miscCard.Controls.Add(dc);
            View.Stack.Add(_miscCard);
        }

        private ToggleSwitch AddToggleRow(Card card, int y, string title, string desc, bool value, EventHandler onChanged)
        {
            card.Controls.Add(L(title, Theme.Bold, Theme.Text, 22, y));
            card.Controls.Add(L(desc, Theme.Small, Theme.TextMuted, 22, y + 19));
            ToggleSwitch t = new ToggleSwitch();
            t.Checked = value;
            t.Anchor = AnchorStyles.Top | AnchorStyles.Right;
            t.Location = new Point(DesignWidth - 22 - 46, y + 4);
            t.CheckedChanged += onChanged;
            card.Controls.Add(t);
            return t;
        }

        private void ApplyModes()
        {
            for (int i = 0; i < _modeButtons.Length; i++)
            {
                string tag = new string[] { "disabled", "all", "tcp", "udp" }[i];
                _modeButtons[i].Kind = tag == _mode ? ButtonKind.Primary : ButtonKind.Secondary;
                _modeButtons[i].Invalidate();
            }
        }

        private void ApplyGameFilter()
        {
            if (Shell.Install == null || !Shell.Install.IsValid) return;
            string tcp = _tcpBox.Text.Trim();
            string udp = _udpBox.Text.Trim();
            if (!GameFilterState.IsRangeValid(tcp) || !GameFilterState.IsRangeValid(udp))
            {
                Shell.Notify("Неверный формат портов. Пример: 1024-65535 или 1024-1934,1936-65535", LogLevel.Error);
                return;
            }
            try
            {
                GameFilterState gf = new GameFilterState();
                gf.Mode = _mode;
                gf.TcpRange = tcp;
                gf.UdpRange = udp;
                gf.Save(Shell.Install.Root);
                Shell.Notify("Настройки игрового фильтра сохранены. Перезапустите bypass, чтобы применить.", LogLevel.Ok);
                Shell.RefreshState(false);
            }
            catch (Exception ex) { Shell.Notify("Не удалось сохранить: " + ex.Message, LogLevel.Error); }
        }

        private void ApplyIpsetMode(string mode)
        {
            if (Shell.Install == null || !Shell.Install.IsValid) return;
            if (!Shell.RequireAdmin("Переключение IPSet")) return;
            string msg = Shell.Ops.SetIpsetMode(mode);
            Shell.Notify(msg + " Перезапустите bypass, чтобы применить.", LogLevel.Ok);
            Shell.RefreshState(false);
            RefreshState();
        }

        private void UpdateIpset()
        {
            if (Shell.Install == null || !Shell.Install.IsValid) return;
            _btnIpsetUpdate.Enabled = false;
            _ipsetProgress.Value = 0;
            Shell.RunBackground(delegate()
            {
                string msg = Shell.Ops.UpdateIpset(delegate(int p, long read, long total)
                {
                    Shell.Ui(delegate() { _ipsetProgress.Value = p < 0 ? 30 : p; });
                });
                Shell.Ui(delegate()
                {
                    _btnIpsetUpdate.Enabled = true;
                    _ipsetProgress.Value = 0;
                    Shell.Notify(msg, msg.StartsWith("Ошибка") ? LogLevel.Error : LogLevel.Ok);
                    Shell.RefreshState(false);
                    RefreshState();
                });
            }, null);
        }

        private void ReloadFakes()
        {
            if (Shell.Install == null || !Shell.Install.IsValid) return;
            System.Collections.Generic.List<ZapretOps.FakeFile> fakes = Shell.Ops.ListFakes();
            System.Collections.Generic.List<string> names = new System.Collections.Generic.List<string>();
            foreach (ZapretOps.FakeFile f in fakes) names.Add(f.Name);
            string d = Shell.Ops.FindActiveFake("ACTIVE_DISCORD_UDP.bin");
            string g = Shell.Ops.FindActiveFake("ACTIVE_GAME_UDP.bin");
            _discordFake.SetItems(names, d);
            _gameFake.SetItems(names, g);
            _fakeInfo.Text = string.Format("Сейчас: Discord — {0}, игровой — {1}. Файлов в bin: {2}", d, g, fakes.Count);
        }

        private void ReplaceFake(string activeFile, DarkSelect combo)
        {
            if (Shell.Install == null || !Shell.Install.IsValid) return;
            if (combo.SelectedIndex < 0) return;
            string name = combo.SelectedItem;
            string src = Path.Combine(Shell.Install.BinDir, name + ".bin");
            if (!Dlg.Confirm(this.View.FindForm(), "Заменить фейк?",
                string.Format("{0} будет заменён содержимым файла {1}.bin. Перезапустите bypass, чтобы применить.", activeFile, name), "Заменить")) return;
            string msg = Shell.Ops.ReplaceActiveFake(activeFile, src);
            Shell.Notify(msg, LogLevel.Ok);
            ReloadFakes();
        }

        public override void OnShow()
        {
            ReloadFakes();
            RefreshState();
        }

        public override void RefreshState()
        {
            if (Shell.Install == null || !Shell.Install.IsValid) return;
            RuntimeState st = Shell.State;
            if (st == null) return;
            _suppress = true;
            try
            {
                _mode = st.GameFilter.Mode;
                _tcpBox.Text = st.GameFilter.TcpRange;
                _udpBox.Text = st.GameFilter.UdpRange;
                ApplyModes();

                _ipsetMode = st.IpsetMode;
                for (int i = 0; i < _ipsetButtons.Length; i++)
                {
                    string tag = Convert.ToString(_ipsetButtons[i].Tag);
                    _ipsetButtons[i].Kind = tag == _ipsetMode ? ButtonKind.Primary : ButtonKind.Secondary;
                    _ipsetButtons[i].Invalidate();
                }
                long backup = Shell.Ops.CountListLines(Shell.Install.IpsetBackup);
                _ipsetInfo.Text = string.Format("Текущий режим: {0}. Строк в рабочем файле: {1}, в резервном: {2}.",
                    ZapretOps.IpsetModeText(_ipsetMode), st.IpsetLines, backup);
                _tgManagerUpdate.Checked = Shell.Config.GetBool("CheckUpdatesOnStartup", true);
                RefreshStartupStatus();
                _tgCloseToTray.Checked = Shell.Config.GetBool("CloseToTray", true);
                _tgAutoZapret.Checked = Shell.Config.GetBool("AutostartZapret", true);
                _tgAutoTg.Checked = Shell.Config.GetBool("AutostartTgProxy", true);
                _tgMinimize.Checked = Shell.Config.GetBool("MinimizeToTray", true);
            }
            finally { _suppress = false; }
        }
    }
}

