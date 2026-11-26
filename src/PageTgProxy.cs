using System;
using System.Drawing;
using System.IO;
using System.Windows.Forms;

namespace ZapretManager
{
    /// <summary>Страница управления TG WS Proxy (локальный MTProto-прокси для Telegram).</summary>
    public sealed class TgProxyPage : PageBase
    {
        private Card _stateCard;
        private Card _linkCard;
        private Card _updateCard;
        private Card _autoCard;

        private Label _stateTitle;
        private Label _stateLine1;
        private Label _stateLine2;
        private Label _stateLine3;
        private RoundButton _btnMain;
        private RoundButton _btnRestart;

        private InputBox _linkBox;
        private Label _linkHint;
        private RoundButton _btnTgOpen;
        private RoundButton _btnTgCopy;

        private Label _versionLine;
        private Label _latestLine;
        private RoundButton _btnCheck;
        private RoundButton _btnInstall;
        private LineProgress _progress;
        private Label _stage;

        private ToggleSwitch _tgAutostart;
        private ToggleSwitch _tgQuiet;
        private Label _quietStatus;
        private RoundButton _btnPrepare;
        private LineProgress _quietProgress;
        private bool _quietBusy;
        private Label _autoInfo;
        private bool _suppress;
        private bool _working;
        private ReleaseInfo _latest;

        public TgProxyPage(Shell shell) : base(shell) { }

        public override string Key { get { return "tgproxy"; } }
        public override string Title { get { return "TG-прокси"; } }
        public override string Subtitle { get { return "Ускорение Telegram через WebSocket (TG WS Proxy)"; } }
        public override string Icon { get { return "\uE8BD"; } }

        public override void Build()
        {
            BuildStateCard();
            BuildLinkCard();
            BuildUpdateCard();
            BuildAutoCard();
            BuildModeCard();
        }

        private void BuildStateCard()
        {
            _stateCard = NewCard("Состояние", 214);
            _stateTitle = L("Проверка…", Theme.Big, Theme.Text, 20, 40);
            _stateCard.Controls.Add(_stateTitle);
            _stateLine1 = L("", Theme.Base, Theme.TextDim, 22, 86);
            _stateCard.Controls.Add(_stateLine1);
            _stateLine2 = L("", Theme.Small, Theme.TextMuted, 22, 108);
            _stateCard.Controls.Add(_stateLine2);
            _stateLine3 = new Label();
            _stateLine3.AutoSize = false;
            _stateLine3.AutoEllipsis = true;
            _stateLine3.Location = new Point(22, 128);
            _stateLine3.Size = new Size(DesignWidth - 60, 16);
            _stateLine3.Font = Theme.Small;
            _stateLine3.ForeColor = Theme.TextMuted;
            _stateLine3.BackColor = Color.Transparent;
            _stateCard.Controls.Add(_stateLine3);

            _btnMain = Btn("Запустить", ButtonKind.Primary, 132);
            _btnMain.Height = 36;
            _btnMain.Anchor = AnchorStyles.Top | AnchorStyles.Right;
            _btnMain.Location = new Point(DesignWidth - 20 - 132, 50);
            _btnMain.Click += delegate { ToggleProxy(); };
            _stateCard.Controls.Add(_btnMain);

            _btnRestart = Btn("Перезапустить", ButtonKind.Secondary, 140);
            _btnRestart.Height = 36;
            _btnRestart.Anchor = AnchorStyles.Top | AnchorStyles.Right;
            _btnRestart.Location = new Point(DesignWidth - 20 - 140 - 144, 50);
            _btnRestart.Click += delegate { RestartProxy(); };
            _stateCard.Controls.Add(_btnRestart);

            View.Stack.Add(_stateCard);
        }

        private void BuildLinkCard()
        {
            _linkCard = NewCard("Подключение Telegram", 200);
            _linkHint = WrapLabel("Прокси работает локально: Telegram → 127.0.0.1:1443 → WebSocket → DC Telegram. Кнопка «Открыть в Telegram» подключает прокси сама; вручную — Настройки → Продвинутые → Тип подключения → Прокси → MTProto. У TG WS Proxy своя иконка в трее — это его штатный интерфейс.",
                Theme.Small, Theme.TextMuted, 22, 40, DesignWidth - 250);
            _linkCard.Controls.Add(_linkHint);

            _linkBox = new InputBox();
            _linkBox.Location = new Point(22, 112);
            _linkBox.Width = DesignWidth - 44 - 200 - 12;
            _linkBox.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right;
            _linkBox.Box.ReadOnly = true;
            _linkCard.Controls.Add(_linkBox);

            _btnTgOpen = Btn("Открыть в Telegram", ButtonKind.Primary, 200);
            _btnTgOpen.Anchor = AnchorStyles.Top | AnchorStyles.Right;
            _btnTgOpen.Location = new Point(DesignWidth - 20 - 200, 112);
            _btnTgOpen.Height = 34;
            _btnTgOpen.Click += delegate { OpenInTelegram(); };
            _linkCard.Controls.Add(_btnTgOpen);

            _btnTgCopy = Btn("Скопировать ссылку", ButtonKind.Ghost, 170);
            _btnTgCopy.Location = new Point(22, 154);
            _btnTgCopy.Click += delegate { CopyLink(); };
            _linkCard.Controls.Add(_btnTgCopy);
            View.Stack.Add(_linkCard);
        }
        private void BuildUpdateCard()
        {
            _updateCard = NewCard("Установка и обновление", 214);
            _versionLine = L("", Theme.Base, Theme.TextDim, 22, 42);
            _updateCard.Controls.Add(_versionLine);
            _latestLine = L("", Theme.Small, Theme.TextMuted, 22, 64);
            _updateCard.Controls.Add(_latestLine);

            _btnCheck = Btn("Проверить обновления", ButtonKind.Secondary, 205);
            _btnCheck.Location = new Point(22, 94);
            _btnCheck.Click += delegate { CheckUpdates(); };
            _updateCard.Controls.Add(_btnCheck);

            _btnInstall = Btn("Скачать и установить", ButtonKind.Primary, 200);
            _btnInstall.Location = new Point(237, 94);
            _btnInstall.Click += delegate { InstallLatest(); };
            _updateCard.Controls.Add(_btnInstall);

            RoundButton folder = Btn("Открыть папку", ButtonKind.Ghost, 150);
            folder.Location = new Point(447, 94);
            folder.Click += delegate
            {
                TgProxyState st = TgProxy.GetState(Shell.Config);
                if (st.Dir.Length > 0) Native.OpenFolder(st.Dir);
                else Native.OpenFolder(TgProxy.DefaultDir);
            };
            _updateCard.Controls.Add(folder);

            RoundButton config = Btn("config.json", ButtonKind.Ghost, 130);
            config.Location = new Point(607, 94);
            config.Click += delegate { Native.SelectInExplorer(TgProxy.ConfigPathForApp); };
            _updateCard.Controls.Add(config);

            _progress = new LineProgress();
            _progress.Location = new Point(22, 150);
            _progress.Width = DesignWidth - 44;
            _progress.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right;
            _updateCard.Controls.Add(_progress);

            _stage = L("", Theme.Small, Theme.TextMuted, 22, 164);
            _updateCard.Controls.Add(_stage);
            View.Stack.Add(_updateCard);
        }

        private void BuildModeCard()
        {
            Card card = NewCard("Режим запуска", 236);
            card.Controls.Add(L("Без иконки в трее (из исходников)", Theme.Base, Theme.Text, 22, 44));

            _quietStatus = L("", Theme.Small, Theme.TextMuted, 22, 88);
            card.Controls.Add(_quietStatus);

            card.Controls.Add(WrapLabel("Тихий режим запускает официальные исходники Flowseal напрямую: окна нет, значка в трее нет, процесс один. Нужен установленный Python 3.9+ и разовая подготовка (исходники и ~30 МБ зависимостей).",
                Theme.Small, Theme.TextDim, 22, 112, DesignWidth - 120));

            _tgQuiet = new ToggleSwitch();
            _tgQuiet.Anchor = AnchorStyles.Top | AnchorStyles.Right;
            _tgQuiet.Location = new Point(DesignWidth - 22 - 46, 42);
            _tgQuiet.CheckedChanged += delegate
            {
                if (_suppress) return;
                if (_tgQuiet.Checked && !TgSource.IsPrepared)
                {
                    if (Dlg.Confirm(this.View.FindForm(), "Подготовить тихий режим?",
                        "Для запуска без трея нужны Python 3.9+ и разовая установка зависимостей (около 30 МБ, 1-2 минуты).\n\nПродолжить?", "Подготовить"))
                    {
                        PrepareQuiet();
                    }
                    else
                    {
                        _suppress = true;
                        _tgQuiet.Checked = false;
                        _suppress = false;
                    }
                    return;
                }
                TgProxy.SetRunMode(Shell.Config, _tgQuiet.Checked ? "quiet" : "exe");
                ApplyModeSwitch();
            };
            card.Controls.Add(_tgQuiet);

            _btnPrepare = Btn("Подготовить", ButtonKind.Secondary, 170);
            _btnPrepare.Location = new Point(22, 168);
            _btnPrepare.Click += delegate { PrepareQuiet(); };
            card.Controls.Add(_btnPrepare);

            _quietProgress = new LineProgress();
            _quietProgress.Location = new Point(22, 210);
            _quietProgress.Width = DesignWidth - 44;
            _quietProgress.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right;
            _quietProgress.Visible = false;
            card.Controls.Add(_quietProgress);

            View.Stack.Add(card);
        }

        private void ApplyModeSwitch()
        {
            TgProxyState st = TgProxy.GetState(Shell.Config);
            if (!st.Running) return;
            Shell.RunBackground(delegate()
            {
                TgProxy.Stop(st.Dir);
                System.Threading.Thread.Sleep(1200);
                bool ok;
                string message = TgProxy.Start(Shell.Config, out ok);
                Shell.Ui(delegate()
                {
                    Shell.Notify(message, ok ? LogLevel.Ok : LogLevel.Error);
                    RefreshState();
                });
            }, null);
        }

        private void PrepareQuiet()
        {
            if (_quietBusy) return;

            _quietBusy = true;
            _btnPrepare.Enabled = false;
            _quietProgress.Visible = true;
            _quietProgress.Value = 2;
            _quietStatus.Text = "Подготовка: скачиваю портативный Python…";
            Shell.RunBackground(delegate()
            {
                bool ok;
                string message = TgSource.Prepare(TgSource.LatestRef(), delegate(int percent, string stage)
                {
                    Shell.Ui(delegate()
                    {
                        _quietProgress.Value = percent;
                        _quietStatus.Text = stage;
                    });
                }, out ok);

                Shell.Ui(delegate()
                {
                    _quietBusy = false;
                    _btnPrepare.Enabled = true;
                    _quietProgress.Visible = false;
                    _quietProgress.Value = 0;
                    _quietStatus.Text = message;
                    _quietStatus.ForeColor = ok ? Theme.Success : Theme.Warn;
                    Shell.Notify(ok ? "Тихий режим готов." : message, ok ? LogLevel.Ok : LogLevel.Error);
                    if (ok)
                    {
                        _suppress = true;
                        _tgQuiet.Checked = true;
                        _suppress = false;
                        TgProxy.SetRunMode(Shell.Config, "quiet");
                        ApplyModeSwitch();
                    }
                    RefreshState();
                });
            }, null);
        }

        private void BuildAutoCard()
        {
            _autoCard = NewCard("Автозапуск", 132);
            _autoCard.Controls.Add(L("Запускать TG WS Proxy вместе с менеджером", Theme.Bold, Theme.Text, 22, 42));
            _autoCard.Controls.Add(L("Срабатывает при старте менеджера, в том числе при автозапуске Windows", Theme.Small, Theme.TextMuted, 22, 61));
            _tgAutostart = new ToggleSwitch();
            _tgAutostart.Anchor = AnchorStyles.Top | AnchorStyles.Right;
            _tgAutostart.Location = new Point(DesignWidth - 22 - 46, 46);
            _tgAutostart.CheckedChanged += delegate
            {
                if (_suppress) return;
                Shell.Config.SetBool("AutostartTgProxy", _tgAutostart.Checked);
                Shell.Config.Save();
                Shell.Notify(_tgAutostart.Checked ? "TG WS Proxy будет запускаться вместе с менеджером." : "Автозапуск TG WS Proxy отключён.", LogLevel.Ok);
            };
            _autoCard.Controls.Add(_tgAutostart);

            _autoInfo = L("", Theme.Small, Theme.TextMuted, 22, 92);
            _autoCard.Controls.Add(_autoInfo);
            View.Stack.Add(_autoCard);
        }

        private void ToggleProxy()
        {
            TgProxyState st = TgProxy.GetState(Shell.Config);
            if (!st.Installed)
            {
                Shell.Notify("TG WS Proxy не установлен — нажмите «Скачать и установить».", LogLevel.Warn);
                return;
            }
            if (st.Running)
            {
                Shell.RunOperation(delegate() { return TgProxy.Stop(st.Dir); }, delegate(string msg)
                {
                    Shell.Notify(msg, LogLevel.Info);
                    Shell.RefreshState(false);
                    RefreshState();
                });
            }
            else
            {
                bool ok = false;
                Shell.RunOperation(delegate() { return TgProxy.Start(Shell.Config, out ok); }, delegate(string msg)
                {
                    Shell.Notify(msg, ok ? LogLevel.Ok : LogLevel.Error);
                    Shell.RefreshState(false);
                    RefreshState();
                });
            }
        }

        private void RestartProxy()
        {
            TgProxyState st = TgProxy.GetState(Shell.Config);
            bool ok = false;
            Shell.RunOperation(delegate()
            {
                TgProxy.Stop(st.Dir);
                System.Threading.Thread.Sleep(800);
                return TgProxy.Start(Shell.Config, out ok);
            }, delegate(string msg)
            {
                Shell.Notify(msg, ok ? LogLevel.Ok : LogLevel.Error);
                Shell.RefreshState(false);
                RefreshState();
            });
        }

        private void OpenInTelegram()
        {
            string msg = TgProxy.OpenInTelegram(Shell.Config);
            TgProxyState st = TgProxy.GetState(Shell.Config);
            if (string.IsNullOrEmpty(st.Link) && !st.Running)
                Shell.Notify("Сначала запустите TG WS Proxy — он сформирует секрет.", LogLevel.Warn);
            else
                Shell.Notify(msg, LogLevel.Ok);
        }

        private void CopyLink()
        {
            TgProxyState st = TgProxy.GetState(Shell.Config);
            if (string.IsNullOrEmpty(st.Link)) { Shell.Notify("Ссылка ещё не сформирована — запустите прокси.", LogLevel.Warn); return; }
            try
            {
                Clipboard.SetText(st.Link);
                Shell.Notify("Ссылка скопирована. Вставьте её в «Избранное» Telegram и нажмите.", LogLevel.Ok);
            }
            catch (Exception ex) { Shell.Notify("Не удалось скопировать: " + ex.Message, LogLevel.Error); }
        }

        private void CheckUpdates()
        {
            if (_working) return;
            _btnCheck.Enabled = false;
            _stage.Text = "Проверка обновлений…";
            Shell.RunBackground(delegate()
            {
                ReleaseInfo rel = null;
                string error = "";
                try { rel = TgProxy.CheckLatest(); }
                catch (Exception ex) { error = ex.Message; }
                Shell.Ui(delegate()
                {
                    _btnCheck.Enabled = true;
                    if (rel == null)
                    {
                        _stage.Text = "Не удалось проверить обновления: " + error;
                        Shell.Notify("Не удалось проверить обновления TG WS Proxy: " + error, LogLevel.Error);
                        return;
                    }
                    _latest = rel;
                    TgProxyState st = TgProxy.GetState(Shell.Config);
                    bool available = TgProxy.UpdateAvailable(st, rel);
                    _latestLine.Text = available
                        ? "Доступна новая версия: " + rel.Tag
                        : "Установлена актуальная версия.";
                    _latestLine.ForeColor = available ? Theme.Warn : Theme.Success;
                    _btnInstall.Enabled = true;
                    Shell.Notify(available ? "Доступна версия TG WS Proxy " + rel.Tag : "TG WS Proxy обновлён до последней версии.", available ? LogLevel.Warn : LogLevel.Ok);
                });
            }, null);
        }

        private void InstallLatest()
        {
            if (_working) return;
            if (!Shell.RequireAdmin("Установка TG WS Proxy")) return;
            _working = true;
            _btnInstall.Enabled = false;
            _btnCheck.Enabled = false;
            _progress.Value = 0;

            string targetDir = Shell.Config.Get("TgDir", TgProxy.DefaultDir);
            TgProxyState st = TgProxy.GetState(Shell.Config);
            if (st.Dir.Length > 0) targetDir = st.Dir;

            Shell.RunBackground(delegate()
            {
                string error = "";
                string tag = "";
                try
                {
                    ReleaseInfo rel = _latest ?? TgProxy.CheckLatest();
                    tag = rel.Tag;
                    string msg;
                    bool ok;
                    msg = TgProxy.Install(Shell.Config, tag, targetDir, delegate(int percent, string stage)
                    {
                        Shell.Ui(delegate() { _progress.Value = percent; _stage.Text = stage; });
                    }, out ok);
                    error = ok ? "" : msg;
                    if (ok) Log.Ok(msg);
                }
                catch (Exception ex) { error = ex.Message; }
                Shell.Ui(delegate()
                {
                    _working = false;
                    _btnInstall.Enabled = true;
                    _btnCheck.Enabled = true;
                    _progress.Value = 0;
                    if (error.Length > 0)
                    {
                        _stage.Text = "Ошибка: " + error;
                        Shell.Notify("Ошибка установки TG WS Proxy: " + error, LogLevel.Error);
                    }
                    else
                    {
                        _stage.Text = "Установлено.";
                        Shell.Notify("TG WS Proxy " + tag + " установлен.", LogLevel.Ok);
                    }
                    RefreshState();
                    Shell.RefreshState(false);
                });
            }, null);
        }

        public override void OnShow()
        {
            RefreshState();
        }

        public override void RefreshState()
        {
            TgProxyState st = TgProxy.GetState(Shell.Config);
            _suppress = true;
            try
            {
                if (!st.Installed)
                {
                    _stateTitle.Text = "Не установлен";
                    _stateTitle.ForeColor = Theme.TextMuted;
                    _stateLine1.Text = "Нажмите «Скачать и установить» ниже — программа скачает официальный TgWsProxy_windows.exe.";
                    _stateLine2.Text = "Прокси нужен для ускорения Telegram; zapret и TG WS Proxy работают независимо.";
                    _stateLine3.Text = "";
                    _btnMain.Text = "Запустить";
                    _btnMain.Kind = ButtonKind.Primary;
                }
                else if (st.Running)
                {
                    _stateTitle.Text = st.PortListening ? "Прокси работает" : "Запускается…";
                    _stateTitle.ForeColor = st.PortListening ? Theme.Success : Theme.Warn;
                    _stateLine1.Text = string.Format("{0}:{1}   •   PID {2}{3}", st.Host, st.Port,
                        st.Pid, st.Started.HasValue ? "   •   запущен " + OverviewPage.FormatUptime(st.Started) + " назад" : "");
                    _stateLine2.Text = string.Format("Версия {0}   •   секрет {1}{2}",
                        st.Version.Length > 0 ? st.Version : "неизвестна",
                        st.Secret.Length > 0 ? "получен" : "ещё не создан",
                        st.InstanceCount > 2 ? string.Format("   •   процессов: {0} (лишние мешают)", st.InstanceCount) : "");
                    _stateLine2.ForeColor = st.InstanceCount > 2 ? Theme.Warn : Theme.TextMuted;
                    _stateLine3.Text = st.Dir;
                    _btnMain.Text = "Остановить";
                    _btnMain.Kind = ButtonKind.Danger;
                }
                else
                {
                    _stateTitle.Text = "Остановлен";
                    _stateTitle.ForeColor = Theme.TextMuted;
                    _stateLine1.Text = string.Format("Версия {0}", st.Version.Length > 0 ? st.Version : "неизвестна");
                    _stateLine2.Text = st.ConfigError.Length > 0 ? "Конфигурация: " + st.ConfigError : "Прокси не слушает порт — Telegram сейчас идёт напрямую.";
                    _stateLine3.Text = st.Dir;
                    _btnMain.Text = "Запустить";
                    _btnMain.Kind = ButtonKind.Primary;
                }
                _btnMain.Invalidate();
                _btnRestart.Enabled = st.Installed;

                _linkBox.Box.Text = !st.Installed
                    ? "TG WS Proxy не установлен"
                    : (st.Link.Length > 0 ? st.Link : "Секрет появится после первого запуска прокси");
                _btnTgOpen.Enabled = st.Installed;
                        _btnTgCopy.Enabled = st.Link.Length > 0;
            if (_linkHint != null)
            {
                _linkHint.Text = string.Format("Прокси работает локально: Telegram → {0}:{1} → WebSocket → DC Telegram. Кнопка «Открыть в Telegram» подключает прокси сама; вручную — Настройки → Продвинутые → Тип подключения → Прокси → MTProto. У TG WS Proxy своя иконка в трее — это его штатный интерфейс.",
                    st.Host, st.Port);
                _linkHint.Height = TextRenderer.MeasureText(_linkHint.Text, _linkHint.Font, new Size(_linkHint.Width, 2000), TextFormatFlags.WordBreak).Height + 4;
            }

                _versionLine.Text = st.Installed
                    ? string.Format("Установленная версия: {0}", st.Version.Length > 0 ? st.Version : "неизвестна")
                    : "TG WS Proxy ещё не установлен";
                _versionLine.ForeColor = st.Installed ? Theme.Text : Theme.TextMuted;
                if (_latest == null && TgProxy.LastKnownLatest != null) _latest = TgProxy.LastKnownLatest;
                if (_latest != null)
                {
                    bool available = TgProxy.UpdateAvailable(st, _latest);
                    _latestLine.Text = available ? "Доступна новая версия: " + _latest.Tag : "Установлена актуальная версия.";
                    _latestLine.ForeColor = available ? Theme.Warn : Theme.Success;
                }
                else if (_latestLine.Text.Length == 0)
                {
                    _latestLine.Text = "Нажмите «Проверить обновления», чтобы узнать о новой версии.";
                }

                _tgAutostart.Checked = Shell.Config.GetBool("AutostartTgProxy", true);

                _suppress = true;
                _tgQuiet.Checked = TgProxy.RunMode(Shell.Config) == "quiet";
                _suppress = false;
                if (_quietBusy) { }
                else if (TgSource.IsPrepared)
                {
                    _quietStatus.Text = "Подготовлено: " + TgSource.PreparedVersion() + (st.QuietMode && st.Running ? "  •  работает без трея (PID " + st.Pid + ")" : "  •  можно включить");
                    _quietStatus.ForeColor = Theme.TextMuted;
                    _btnPrepare.Text = "Обновить исходники";
                    _btnPrepare.Fit(10);
                }
                else
                {
                    _quietStatus.Text = "Тихий режим не подготовлен: нажмите «Подготовить» — портативный Python (≈10 МБ) скачается автоматически.";
                    _quietStatus.ForeColor = Theme.TextMuted;
                }
                _autoInfo.Text = Shell.Config.GetBool("AutostartZapret", true)
                    ? "Менеджер при автозапуске поднимает и zapret, и TG WS Proxy."
                    : "Менеджер при автозапуске поднимает только TG WS Proxy (zapret отключён в «Настройки»).";
            }
            finally { _suppress = false; }
        }
    }
}
