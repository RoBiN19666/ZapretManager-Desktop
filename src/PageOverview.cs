using System;
using System.Drawing;
using System.Windows.Forms;

namespace ZapretManager
{
    public sealed class OverviewPage : PageBase
    {
        private Card _installCard;
        private Card _statusCard;
        private Card _updateCard;
        private Card _tgCard;
        private Label _tgTitle;
        private Label _tgLine;
        private RoundButton _tgBtn;
        private Card _quickCard;
        private Card _logCard;

        private Label _stateTitle;
        private Label _stateLine1;
        private Label _stateLine2;
        private RoundButton _btnMain;
        private RoundButton _btnRestart;

        private Label _updateTitle;
        private Label _updateLine;
        private RoundButton _btnCheckUpdate;
        private RoundButton _btnGoUpdate;
        private Label _selfLine;
        private RoundButton _btnSelfGo;

        private ToggleSwitch _tgGame;
        private ToggleSwitch _tgIpSet;
        private ToggleSwitch _tgAutoUpdate;
        private LogView _log;
        private bool _suppress;

        private Card _warnCard;
        private Label _warnText;
        private RoundButton _warnFix;
        private RoundButton _warnOpen;

        public OverviewPage(Shell shell) : base(shell) { }

        public override string Key { get { return "overview"; } }
        public override string Title { get { return "Обзор"; } }
        public override string Subtitle { get { return "Состояние защиты, быстрые действия и журнал"; } }
        public override string Icon { get { return "\uE80F"; } }

        public override void Build()
        {
            BuildInstallCard();
            BuildWarnCard();
            BuildStatusCard();
            BuildUpdateCard();
            BuildTgCard();
            BuildQuickCard();
            BuildLogCard();
            View.Stack.Add(_warnCard);
            View.Stack.Add(_statusCard);
            View.Stack.Add(_updateCard);
            View.Stack.Add(_tgCard);
            View.Stack.Add(_quickCard);
            View.Stack.Add(_logCard);
            View.Stack.Add(_installCard);
            Shell.UpdateStateChanged += RefreshState;
            Log.Appended += OnLog;
        }

        private void BuildWarnCard()
        {
            _warnCard = NewCard("Внимание: служба zapret установлена из другой папки", 150);
            _warnCard.AccentStripe = Theme.Warn;
            _warnText = WrapLabel("", Theme.Small, Theme.TextDim, 22, 40, DesignWidth - 60);
            _warnCard.Controls.Add(_warnText);

            _warnFix = Btn("Переустановить службу из этой папки", ButtonKind.Secondary, 290);
            _warnFix.Location = new Point(22, 88);
            _warnFix.Click += delegate { ReinstallServiceHere(); };
            _warnCard.Controls.Add(_warnFix);

            _warnOpen = Btn("Открыть папку службы", ButtonKind.Ghost, 180);
            _warnOpen.Location = new Point(324, 88);
            _warnOpen.Click += delegate
            {
                string root = Shell.Ops.ServiceInstallRoot;
                if (!string.IsNullOrEmpty(root)) Native.OpenFolder(root);
            };
            _warnCard.Controls.Add(_warnOpen);
            _warnCard.Visible = false;
        }

        /// <summary>Раскладывает карточку предупреждения под фактическую высоту текста.</summary>
        private void LayoutWarnCard()
        {
            if (_warnText == null) return;
            _warnText.Width = DesignWidth - 60;
            int height = TextRenderer.MeasureText(_warnText.Text, _warnText.Font, new Size(_warnText.Width, 4000), TextFormatFlags.WordBreak).Height + 4;
            _warnText.Height = Math.Max(18, height);
            int buttonsY = 40 + _warnText.Height + 10;
            if (_warnFix != null) _warnFix.Location = new Point(22, buttonsY);
            if (_warnOpen != null) _warnOpen.Location = new Point(324, buttonsY);
            _warnCard.Height = buttonsY + 36 + 12;
        }

        private void ReinstallServiceHere()
        {
            if (!Shell.RequireAdmin("Переустановка службы")) return;
            StrategyDef s = StrategyPicker.Pick(Shell);
            if (s == null) return;
            if (!Dlg.Confirm(this.View.FindForm(), "Переустановить службу?",
                string.Format("Служба zapret будет пересоздана из папки «{0}» со стратегией «{1}».", Shell.Install.Root, s.Name), "Переустановить")) return;
            bool ok = false;
            string msg = "";
            Shell.RunOperation(delegate() { msg = Shell.Ops.InstallService(s, out ok); return msg; }, delegate(string message)
            {
                Shell.Notify(message, ok ? LogLevel.Ok : LogLevel.Error);
                Shell.RefreshState(false);
            });
        }

        private void BuildInstallCard()
        {
            _installCard = NewCard("Zapret не найден", 190);
            Card card = _installCard;
            card.AccentStripe = Theme.Warn;
            card.Controls.Add(WrapLabel("Поместите ZapretManager.exe в папку zapret (рядом с service.bat) или укажите папку вручную. Можно также скачать и установить свежую версию автоматически.",
                Theme.Base, Theme.TextDim, 22, 44, DesignWidth - 280));
            card.Paint += delegate(object s, PaintEventArgs e)
            {
                e.Graphics.SmoothingMode = System.Drawing.Drawing2D.SmoothingMode.AntiAlias;
                Brand.DrawLock(e.Graphics, new Rectangle(card.ClientSize.Width - 232, 16, 200, 200), 235);
            };
            RoundButton pick = Btn("Указать папку zapret…", ButtonKind.Secondary, 200);
            pick.Location = new Point(22, 118);
            pick.Click += delegate { PickFolder(); };
            card.Controls.Add(pick);

            RoundButton dl = Btn("Скачать и установить", ButtonKind.Primary, 200);
            dl.Location = new Point(232, 118);
            dl.Click += delegate { Shell.Form.Navigate("update"); };
            card.Controls.Add(dl);
        }

        private void PickFolder()
        {
            OpenFileDialog dlg = new OpenFileDialog();
            dlg.Title = "Выберите service.bat в папке zapret";
            dlg.Filter = "service.bat|service.bat|Все файлы (*.*)|*.*";
            if (DlgHost.Show(dlg, this.View.FindForm()) == DialogResult.OK)
            {
                string root = System.IO.Path.GetDirectoryName(dlg.FileName);
                if (ZapretInstall.LooksValid(root)) Shell.SetRoot(root);
                else Shell.Notify("В выбранной папке нет service.bat и bin\\winws.exe", LogLevel.Error);
            }
        }

        private void BuildStatusCard()
        {
            _statusCard = NewCard("Состояние защиты", 144);
            _stateTitle = L("Проверка…", Theme.Big, Theme.Text, 20, 40);
            _statusCard.Controls.Add(_stateTitle);
            _stateLine1 = L("", Theme.Base, Theme.TextDim, 22, 84);
            _statusCard.Controls.Add(_stateLine1);
            _stateLine2 = L("", Theme.Small, Theme.TextMuted, 22, 106);
            _statusCard.Controls.Add(_stateLine2);

            _btnMain = Btn("Запустить", ButtonKind.Primary, 132);
            _btnMain.Height = 36;
            _btnMain.Anchor = AnchorStyles.Top | AnchorStyles.Right;
            _btnMain.Location = new Point(DesignWidth - 20 - 132, 50);
            _btnMain.Click += delegate { ToggleBypass(); };
            _statusCard.Controls.Add(_btnMain);

            _btnRestart = Btn("Перезапустить", ButtonKind.Secondary, 140);
            _btnRestart.Height = 36;
            _btnRestart.Anchor = AnchorStyles.Top | AnchorStyles.Right;
            _btnRestart.Location = new Point(DesignWidth - 20 - 140 - 144, 50);
            _btnRestart.Click += delegate { RestartBypass(); };
            _statusCard.Controls.Add(_btnRestart);
        }

        private void BuildUpdateCard()
        {
            _updateCard = NewCard("Обновления", 128);
            _updateTitle = L("", Theme.Bold, Theme.Text, 20, 44);
            _updateCard.Controls.Add(_updateTitle);
            _updateLine = L("", Theme.Small, Theme.TextMuted, 20, 66);
            _updateCard.Controls.Add(_updateLine);

            _btnCheckUpdate = Btn("Проверить", ButtonKind.Secondary, 110);
            _btnCheckUpdate.Anchor = AnchorStyles.Top | AnchorStyles.Right;
            _btnCheckUpdate.Location = new Point(DesignWidth - 20 - 110, 48);
            _btnCheckUpdate.Click += delegate { CheckUpdates(); };
            _updateCard.Controls.Add(_btnCheckUpdate);

            _btnGoUpdate = Btn("Обновить сейчас", ButtonKind.Primary, 150);
            _btnGoUpdate.Anchor = AnchorStyles.Top | AnchorStyles.Right;
            _btnGoUpdate.Location = new Point(DesignWidth - 20 - 150 - 120, 48);
            _btnGoUpdate.Visible = false;
            _btnGoUpdate.Click += delegate { Shell.Form.Navigate("update"); };
            _updateCard.Controls.Add(_btnGoUpdate);

            _selfLine = L("", Theme.Small, Theme.Warn, 22, 88);
            _selfLine.Visible = false;
            _updateCard.Controls.Add(_selfLine);

            _btnSelfGo = Btn("Обновить менеджер", ButtonKind.Link, 170);
            _btnSelfGo.Anchor = AnchorStyles.Top | AnchorStyles.Right;
            _btnSelfGo.Location = new Point(DesignWidth - 20 - 170, 84);
            _btnSelfGo.Visible = false;
            _btnSelfGo.Click += delegate { Shell.Form.Navigate("update"); };
            _updateCard.Controls.Add(_btnSelfGo);
        }

        private void BuildTgCard()
        {
            _tgCard = NewCard("Telegram-прокси (TG WS Proxy)", 112);
            _tgTitle = L("", Theme.Bold, Theme.Text, 22, 42);
            _tgCard.Controls.Add(_tgTitle);
            _tgLine = L("", Theme.Small, Theme.TextMuted, 22, 64);
            _tgCard.Controls.Add(_tgLine);

            _tgBtn = Btn("Запустить", ButtonKind.Secondary, 132);
            _tgBtn.Anchor = AnchorStyles.Top | AnchorStyles.Right;
            _tgBtn.Location = new Point(DesignWidth - 20 - 132, 46);
            _tgBtn.Click += delegate { ToggleTgProxy(); };
            _tgCard.Controls.Add(_tgBtn);

            RoundButton open = Btn("Подробнее", ButtonKind.Ghost, 110);
            open.Anchor = AnchorStyles.Top | AnchorStyles.Right;
            open.Location = new Point(DesignWidth - 20 - 132 - 116, 46);
            open.Click += delegate { Shell.Form.Navigate("tgproxy"); };
            _tgCard.Controls.Add(open);
        }

        private void ToggleTgProxy()
        {
            TgProxyState st = TgProxy.GetState(Shell.Config);
            if (!st.Installed)
            {
                Shell.Form.Navigate("tgproxy");
                Shell.Notify("TG WS Proxy не установлен — установите его на странице «TG-прокси».", LogLevel.Warn);
                return;
            }
            if (st.Running)
            {
                Shell.RunOperation(delegate() { return TgProxy.Stop(st.Dir); }, delegate(string message)
                {
                    Shell.Notify(message, LogLevel.Info);
                    Shell.RefreshState(false);
                    RefreshState();
                });
            }
            else
            {
                bool ok = false;
                Shell.RunOperation(delegate() { return TgProxy.Start(Shell.Config, out ok); }, delegate(string message)
                {
                    Shell.Notify(message, ok ? LogLevel.Ok : LogLevel.Error);
                    Shell.RefreshState(false);
                    RefreshState();
                });
            }
        }
        private void BuildQuickCard()
        {
            _quickCard = NewCard("Быстрые настройки", 200);
            _tgGame = AddToggleRow(_quickCard, 42, "Игровой фильтр", "Расширяет обход на порты игр (1024-65535) — для игр и голосового чата", false, delegate
            {
                if (_suppress) return;
                GameFilterState gf = GameFilterState.Load(Shell.Install.Root);
                gf.Mode = _tgGame.Checked ? "all" : "disabled";
                try
                {
                    gf.Save(Shell.Install.Root);
                    Shell.Notify(_tgGame.Checked ? "Игровой фильтр включён (TCP и UDP). Перезапустите bypass." : "Игровой фильтр выключен. Перезапустите bypass.", LogLevel.Ok);
                    Shell.RefreshState(false);
                }
                catch (Exception ex) { Shell.Notify("Не удалось сохранить настройку: " + ex.Message, LogLevel.Error); }
            });

            _tgIpSet = AddToggleRow(_quickCard, 94, "Полный IP-список", "Фильтровать трафик по списку IP-адресов (ipset-all). Выключено — работаем только по доменам", false, delegate
            {
                if (_suppress) return;
                string mode = _tgIpSet.Checked ? "loaded" : "none";
                Shell.RunOperation(delegate() { return Shell.Ops.SetIpsetMode(mode); }, delegate(string message)
                {
                    bool failed = message.StartsWith("Ошибка") || message.StartsWith("Нет ") || message.StartsWith("Неизвестный");
                    Shell.Notify(message + (failed ? "" : " Перезапустите bypass."), failed ? LogLevel.Error : LogLevel.Ok);
                    Shell.RefreshState(false);
                });
            });

            _tgAutoUpdate = AddToggleRow(_quickCard, 146, "Авто-проверка обновлений", "Проверять новые версии при запуске стратегий", false, delegate
            {
                if (_suppress) return;
                SetCheckUpdatesFlag(_tgAutoUpdate.Checked);
            });
        }

        private void SetCheckUpdatesFlag(bool enabled)
        {
            try
            {
                string file = Shell.Install.CheckUpdatesFile;
                if (enabled)
                {
                    System.IO.Directory.CreateDirectory(Shell.Install.UtilsDir);
                    System.IO.File.WriteAllText(file, "ENABLED", new System.Text.UTF8Encoding(false));
                }
                else if (System.IO.File.Exists(file)) System.IO.File.Delete(file);
                Shell.Notify(enabled ? "Авто-проверка обновлений включена." : "Авто-проверка обновлений выключена.", LogLevel.Ok);
            }
            catch (Exception ex) { Shell.Notify("Не удалось изменить флаг: " + ex.Message, LogLevel.Error); }
        }

        private void BuildLogCard()
        {
            _logCard = NewCard("Журнал", 232);
            _log = new LogView();
            _log.Location = new Point(20, 46);
            _log.Size = new Size(DesignWidth - 40, 160);
            _log.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right | AnchorStyles.Bottom;
            _logCard.Controls.Add(_log);
            _logCard.Resize += delegate
            {
                _log.Size = new Size(_logCard.ClientSize.Width - Theme.Px(40), Math.Max(Theme.Px(40), _logCard.ClientSize.Height - Theme.Px(66)));
            };
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

        private void OnLog(LogLevel level, string text)
        {
            if (_log == null) return;
            // Log.Write может вызываться из фонового потока: RichTextBox трогаем только в UI.
            Shell.Ui(delegate()
            {
                if (_log == null || _log.IsDisposed) return;
                _log.Append(level, text);
            });
        }

        public override void OnShow()
        {
            RefreshState();
        }

        public override void OnDispose()
        {
            Shell.UpdateStateChanged -= RefreshState;
            Log.Appended -= OnLog;
        }

        public override void RefreshState()
        {
            bool valid = Shell.Install != null && Shell.Install.IsValid;
            _installCard.Visible = !valid;
            _statusCard.Visible = valid;
            _updateCard.Visible = valid;
            _quickCard.Visible = valid;
            _logCard.Visible = valid;
            bool mismatch = valid && Shell.Ops.ServicePointsElsewhere;
            bool missing = valid && Shell.Ops.ServicePathMissing;
            _warnCard.Visible = mismatch || missing;
            if (mismatch || missing)
            {
                string serviceRoot = Shell.Ops.ServiceInstallRoot;
                if (missing)
                {
                    _warnCard.Header = "Внимание: служба zapret указывает на несуществующую папку";
                    _warnText.Text = string.Format("Служба установлена из папки «{0}», которой больше нет (папку переместили или удалили). После перезагрузки bypass не запустится. Переустановите службу из текущей папки.",
                        string.IsNullOrEmpty(serviceRoot) ? "неизвестно" : serviceRoot);
                }
                else
                {
                    _warnCard.Header = "Внимание: служба zapret установлена из другой папки";
                    _warnText.Text = string.Format("Служба zapret сейчас запущена из папки «{0}», а менеджер управляет папкой «{1}». Запуск стратегий и обновление затронут только вторую. Переустановите службу, чтобы переключиться.",
                        serviceRoot, Shell.Install.Root);
                }
                _warnText.Height = TextRenderer.MeasureText(_warnText.Text, _warnText.Font, new Size(Math.Max(100, _warnText.Width), 2000), TextFormatFlags.WordBreak).Height + 6;
                _warnText.Width = DesignWidth - 60;
                LayoutWarnCard();
                _warnCard.Invalidate();
            }
            View.Stack.PerformLayout();
            if (!valid) return;

            RuntimeState st = Shell.State;
            if (st == null) return;

            _suppress = true;
            try
            {
                if (st.WinwsRunning)
                {
                    _stateTitle.Text = "Защита активна";
                    _stateTitle.ForeColor = Theme.Success;
                    _btnMain.Text = "Остановить";
                    _btnMain.Kind = ButtonKind.Danger;
                }
                else
                {
                    _stateTitle.Text = "Защита не активна";
                    _stateTitle.ForeColor = Theme.TextMuted;
                    _btnMain.Text = "Запустить";
                    _btnMain.Kind = ButtonKind.Primary;
                }
                _btnMain.Invalidate();

                string strat = st.MatchedStrategy.Length > 0 ? st.MatchedStrategy : (st.WinwsRunning ? "пользовательская" : "—");
                string uptime = st.WinwsRunning ? FormatUptime(st.WinwsStarted) : "";
                _stateLine1.Text = st.WinwsRunning
                    ? string.Format("Стратегия: {0}   •   PID {1}{2}", strat, st.WinwsPid, uptime.Length > 0 ? "   •   работает " + uptime : "")
                    : "Bypass не запущен. Выберите стратегию на странице «Стратегии».";

                _stateLine2.Text = string.Format("Служба zapret: {0}   •   IPSet: {1}   •   Игровой фильтр: {2}   •   Автообновление: {3}",
                    st.ServiceInstalled ? st.ServiceStatus : "не установлена",
                    st.IpsetMode == "loaded" ? string.Format("полный список ({0} строк)", st.IpsetLines) : (st.IpsetMode == "any" ? "любые IP" : "без фильтра"),
                    st.GameFilter.ModeText,
                    st.CheckUpdates ? "вкл." : "выкл.");

                TgProxyState tg = TgProxy.GetState(Shell.Config);
                _tgCard.Visible = true;
                if (!tg.Installed)
                {
                    _tgTitle.Text = "TG WS Proxy не установлен";
                    _tgLine.Text = "Установите прокси Telegram на странице «TG-прокси» — 21 МБ, затем подключение одной кнопкой";
                    _tgBtn.Text = "Установить";
                }
                else if (tg.Running)
                {
                    _tgTitle.Text = "Прокси Telegram работает";
                    _tgLine.Text = string.Format("{0}:{1}   •   версия {2}   •   Telegram подключается автоматически", tg.Host, tg.Port, tg.Version.Length > 0 ? tg.Version : "?");
                    _tgBtn.Text = "Остановить";
                }
                else
                {
                    _tgTitle.Text = "Прокси Telegram остановлен";
                    _tgLine.Text = string.Format("Версия {0}   •   {1}", tg.Version.Length > 0 ? tg.Version : "неизвестна", tg.Dir);
                    _tgBtn.Text = "Запустить";
                }

                _tgGame.Checked = st.GameFilter.Mode == "all" || st.GameFilter.Mode == "tcp" || st.GameFilter.Mode == "udp";
                _tgIpSet.Checked = st.IpsetMode == "loaded";
                _tgAutoUpdate.Checked = st.CheckUpdates;

                string localVersion = Shell.LocalVersion;
                _updateTitle.Text = localVersion.Length > 0 ? "Установлена версия zapret " + localVersion : "Версия zapret не определена";
                SelfUpdateInfo su = SelfUpdate.Last;
                bool suAvail = su != null && su.Ok && su.Available;
                _selfLine.Visible = suAvail;
                _btnSelfGo.Visible = suAvail;
                if (suAvail) _selfLine.Text = string.Format("Менеджер: доступна версия {0} (у вас {1})", su.Remote, su.Local);
                UpdateCheckResult u = Shell.UpdateState;
                if (u == null)
                {
                    _updateLine.Text = Shell.Config.GetBool("CheckUpdatesOnStartup", true) ? "Проверка обновлений выполняется…" : "Автоматическая проверка при запуске отключена.";
                    _btnGoUpdate.Visible = false;
                }
                else if (!u.Ok)
                {
                    _updateLine.Text = "Не удалось проверить обновления: " + u.Error;
                    _btnGoUpdate.Visible = false;
                }
                else if (u.UpdateAvailable)
                {
                    string when = u.Release.Published > new DateTime(2000, 1, 1) ? string.Format(" (опубликована {0:dd.MM.yyyy})", u.Release.Published) : "";
                    _updateLine.Text = string.Format("Доступна новая версия {0}{1}. Ваши списки и настройки сохранятся.", u.RemoteVersion, when);
                    _btnGoUpdate.Visible = true;
                }
                else if (u.LocalUnknown)
                {
                    _updateLine.Text = "Не удалось прочитать версию из service.bat — откройте «Обновление» и поставьте нужную версию вручную.";
                    _btnGoUpdate.Visible = true;
                }
                else
                {
                    _updateLine.Text = "У вас последняя версия.";
                    _btnGoUpdate.Visible = false;
                }
            }
            finally { _suppress = false; }
        }

        public static string FormatUptime(DateTime? started)
        {
            if (!started.HasValue) return "";
            TimeSpan t = DateTime.Now - started.Value;
            if (t.TotalSeconds < 0) return "";
            if (t.TotalMinutes < 1) return "меньше минуты";
            if (t.TotalHours < 1) return string.Format("{0} мин", (int)t.TotalMinutes);
            if (t.TotalDays < 1) return string.Format("{0} ч {1} мин", (int)t.TotalHours, t.Minutes);
            return string.Format("{0} д {1} ч", (int)t.TotalDays, t.Hours);
        }

        private StrategyDef CurrentOrDefaultStrategy()
        {
            return StrategyPicker.Pick(Shell);
        }

        private void ToggleBypass()
        {
            RuntimeState st = Shell.State;
            if (st != null && st.WinwsRunning)
            {
                if (!Shell.RequireAdmin("Остановка")) return;
                Shell.RunOperation(delegate() { return Shell.Ops.Stop(); }, delegate(string message)
                {
                    Shell.Notify(message, LogLevel.Ok);
                    Shell.RefreshState(false);
                });
                return;
            }
            if (!Shell.RequireAdmin("Запуск")) return;
            StrategyDef s = CurrentOrDefaultStrategy();
            if (s == null) { Shell.Notify("Стратегии не найдены в папке zapret.", LogLevel.Error); return; }
            ZapretOps.StartResult r = null;
            Shell.RunOperation(delegate() { r = Shell.Ops.Start(s); return r.Message; }, delegate(string message)
            {
                Shell.Notify(message, r != null && r.Ok ? LogLevel.Ok : LogLevel.Error);
                Shell.RefreshState(false);
            });
        }

        private void RestartBypass()
        {
            if (!Shell.RequireAdmin("Перезапуск")) return;
            StrategyDef s = CurrentOrDefaultStrategy();
            if (s == null) { Shell.Notify("Стратегии не найдены.", LogLevel.Error); return; }
            Shell.RunOperation(delegate() { return Shell.Ops.Restart(s); }, delegate(string message)
            {
                Shell.Notify(message, LogLevel.Ok);
                Shell.RefreshState(false);
            });
        }

        private void CheckUpdates()
        {
            _btnCheckUpdate.Enabled = false;
            _updateLine.Text = "Проверка обновлений…";
            string local = Shell.LocalVersion;
            Shell.RunBackground(delegate()
            {
                UpdateCheckResult res = UpdateChecker.Check(local, true);
                Shell.Ui(delegate()
                {
                    _btnCheckUpdate.Enabled = true;
                    Shell.UpdateState = res;
                    Shell.RaiseUpdateStateChanged();
                    if (!res.Ok) Shell.Notify("Не удалось проверить обновления: " + res.Error, LogLevel.Error);
                    else if (res.UpdateAvailable) Shell.Notify(string.Format("Доступна версия {0}", res.RemoteVersion), LogLevel.Warn);
                    else Shell.Notify("У вас последняя версия " + local + ".", LogLevel.Ok);
                    RefreshState();
                });
            }, null);
        }
    }

    public static class StrategyPicker
    {
        public static StrategyDef Pick(Shell shell)
        {
            ZapretInstall inst = shell.Install;
            if (inst == null || !inst.IsValid || inst.Strategies.Count == 0) return null;
            if (shell.State != null && shell.State.MatchedStrategy.Length > 0)
            {
                StrategyDef cur = inst.FindStrategy(shell.State.MatchedStrategy);
                if (cur != null) return cur;
            }
            string last = shell.Config.Get("LastStrategy", "");
            StrategyDef saved = inst.FindStrategy(last);
            if (saved != null) return saved;
            StrategyDef gen = inst.FindStrategy("general");
            return gen != null ? gen : inst.Strategies[0];
        }
    }
}
