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
        private RoundButton _btnTgOpen;
        private RoundButton _btnTgCopy;

        private Label _versionLine;
        private Label _latestLine;
        private RoundButton _btnCheck;
        private RoundButton _btnInstall;
        private LineProgress _progress;
        private Label _stage;

        private ToggleSwitch _tgAutostart;
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
            _linkCard = NewCard("Подключение Telegram", 168);
            _linkCard.Controls.Add(WrapLabel("Прокси работает локально: Telegram → 127.0.0.1:1443 → WebSocket → DC Telegram. Кнопка «Открыть в Telegram» подключает прокси автоматически; вручную — Настройки → Продвинутые → Тип подключения → Прокси → MTProto. У самого TG WS Proxy есть своя иконка в трее — это его штатный интерфейс (логи, настройки, ссылка).",
                Theme.Small, Theme.TextMuted, 22, 40, DesignWidth - 60));

            _linkBox = new InputBox();
            _linkBox.Location = new Point(22, 96);
            _linkBox.Width = DesignWidth - 44 - 200 - 12;
            _linkBox.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right;
            _linkBox.Box.ReadOnly = true;
            _linkCard.Controls.Add(_linkBox);

            _btnTgOpen = Btn("Открыть в Telegram", ButtonKind.Primary, 200);
            _btnTgOpen.Anchor = AnchorStyles.Top | AnchorStyles.Right;
            _btnTgOpen.Location = new Point(DesignWidth - 20 - 200, 96);
            _btnTgOpen.Height = 34;
            _btnTgOpen.Click += delegate { OpenInTelegram(); };
            _linkCard.Controls.Add(_btnTgOpen);

            _btnTgCopy = Btn("Скопировать ссылку", ButtonKind.Ghost, 170);
            _btnTgCopy.Location = new Point(22, 136);
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
                Shell.Notify(TgProxy.Stop(), LogLevel.Info);
            }
            else
            {
                bool ok;
                string msg = TgProxy.Start(Shell.Config, out ok);
                Shell.Notify(msg, ok ? LogLevel.Ok : LogLevel.Error);
            }
            Shell.RefreshState(false);
            RefreshState();
        }

        private void RestartProxy()
        {
            Shell.Notify(TgProxy.Stop(), LogLevel.Info);
            bool ok;
            string msg = TgProxy.Start(Shell.Config, out ok);
            Shell.Notify(msg, ok ? LogLevel.Ok : LogLevel.Error);
            Shell.RefreshState(false);
            RefreshState();
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

                _versionLine.Text = st.Installed
                    ? string.Format("Установленная версия: {0}", st.Version.Length > 0 ? st.Version : "неизвестна")
                    : "TG WS Proxy ещё не установлен";
                _versionLine.ForeColor = st.Installed ? Theme.Text : Theme.TextMuted;
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
                _autoInfo.Text = Shell.Config.GetBool("AutostartZapret", true)
                    ? "Менеджер при автозапуске поднимает и zapret, и TG WS Proxy."
                    : "Менеджер при автозапуске поднимает только TG WS Proxy (zapret отключён в «Настройки»).";
            }
            finally { _suppress = false; }
        }
    }
}
