using System;
using System.Collections.Generic;
using System.Drawing;
using System.Text;
using System.Threading;
using System.Windows.Forms;

namespace ZapretManager
{
    public sealed class SitesPage : PageBase
    {
        private InputBox _input;
        private RoundButton _btnCheck;
        private Label _verdict;
        private Label _steps;
        private LineProgress _progress;
        private DarkEditor _hostsEditor;
        private Label _status;
        private Label _hostsHint;
        private RoundButton _btnProbeHosts;
        private RoundButton _btnToList;
        private RoundButton _btnToHosts;
        private RoundButton _btnCopy;
        private SiteProbe _probe;
        private ToggleSwitch _tgGuard;
        private ToggleSwitch _tgGuardAuto;
        private Label _guardStatus;
        private DarkSelect _procSelect;
        private Label _procStatus;
        private RoundButton _btnAppWatch;
        private RoundButton _btnAppRefresh;
        private bool _appBusy;
        private List<AppEntry> _apps = new List<AppEntry>();
        private bool _working;
        /// <summary>Готовность проверки для служебных снимков экрана.</summary>
        public static bool Ready = true;

        public SitesPage(Shell shell) : base(shell) { }

        public override string Key { get { return "sites"; } }
        public override string Title { get { return "Проверка сайта"; } }
        public override string Subtitle { get { return "Диагностика блокировки и добавление адресов в списки"; } }
        public override string Icon { get { return "\uE774"; } }

        public override void Build()
        {
            BuildCheckCard();
            BuildAppCard();
            BuildHostsCard();
            BuildHelpCard();
            BuildGuardCard();

            if (!string.IsNullOrEmpty(AppInfo.ShotSite))
            {
                Ready = false;
                _input.Text = AppInfo.ShotSite;
                Log.Info("Служебный снимок: проверяем " + AppInfo.ShotSite);
                RunCheck();
            }
        }

        private void BuildCheckCard()
        {
            Card card = NewCard("Проверка сайта", 272);
            _input = new InputBox();
            _input.Location = new Point(22, 42);
            _input.Width = 420;
            _input.Text = "https://";
            card.Controls.Add(_input);

            _btnCheck = Btn("Проверить", ButtonKind.Primary, 130);
            _btnCheck.Location = new Point(452, 40);
            _btnCheck.Click += delegate { RunCheck(); };
            card.Controls.Add(_btnCheck);

            RoundButton open = Btn("Открыть в браузере", ButtonKind.Ghost, 180);
            open.Location = new Point(594, 40);
            open.Click += delegate
            {
                string url;
                string host = SiteDoctor.NormalizeInput(_input.Text, out url);
                if (host.Length == 0) { Shell.Notify("Введите адрес сайта.", LogLevel.Warn); return; }
                Native.OpenUrl(url);
            };
            card.Controls.Add(open);

            _verdict = WrapLabel("Введите адрес сайта и нажмите «Проверить» — менеджер по шагам определит, где рвётся соединение.",
                Theme.Base, Theme.TextDim, 22, 86, DesignWidth - 60);
            card.Controls.Add(_verdict);

            _steps = L("", Theme.Small, Theme.TextMuted, 22, 170);
            card.Controls.Add(_steps);

            _progress = new LineProgress();
            _progress.Location = new Point(22, 240);
            _progress.Width = DesignWidth - 44;
            _progress.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right;
            card.Controls.Add(_progress);

            View.Stack.Add(card);
        }

        private void BuildAppCard()
        {
            Card card = NewCard("Проверка приложения", 206);
            card.Controls.Add(WrapLabel("Игра или программа не запускается? Выберите её в списке и нажмите «Наблюдать»: менеджер соберёт домены, к которым она обращается (журнал DNS), проверит их и добавит заблокированные в ваши списки.",
                Theme.Small, Theme.TextDim, 22, 40, DesignWidth - 60));

            _procSelect = new DarkSelect();
            _procSelect.Location = new Point(22, 92);
            _procSelect.Width = 420;
            card.Controls.Add(_procSelect);

            _btnAppRefresh = Btn("Обновить список", ButtonKind.Secondary, 170);
            _btnAppRefresh.Location = new Point(500, 90);
            _btnAppRefresh.Click += delegate { LoadApps(); };
            card.Controls.Add(_btnAppRefresh);

            _btnAppWatch = Btn("Наблюдать 60 с", ButtonKind.Primary, 180);
            _btnAppWatch.Location = new Point(22, 132);
            _btnAppWatch.Click += delegate { WatchApp(); };
            card.Controls.Add(_btnAppWatch);

            _procStatus = L("Список приложений загружается…", Theme.Small, Theme.TextMuted, 22, 170);
            card.Controls.Add(_procStatus);

            View.Stack.Add(card);
        }

        private void LoadApps()
        {
            _btnAppRefresh.Enabled = false;
            _procStatus.Text = "Собираем список приложений…";
            Shell.RunBackground(delegate()
            {
                List<AppEntry> apps = AppProbe.List();
                Log.Info("Проверка приложения: процессов с окнами — " + apps.Count);
                Shell.Ui(delegate()
                {
                    _btnAppRefresh.Enabled = true;
                    _apps = apps;
                    List<string> names = new List<string>();
                    foreach (AppEntry e in apps) names.Add(e.ToString());
                    if (names.Count == 0)
                    {
                        _procStatus.Text = "Не нашли приложений с окнами — запустите игру и нажмите «Обновить список».";
                        return;
                    }
                    _procSelect.SetItems(names, names[0]);
                    _procStatus.Text = "Приложений в списке: " + names.Count + ". Запустите игру (можно прямо сейчас) и нажмите «Наблюдать 60 с».";
                });
            }, null);
        }

        private void WatchApp()
        {
            if (_appBusy) return;
            int index = _procSelect.SelectedIndex;
            if (index < 0 || index >= _apps.Count) { Shell.Notify("Сначала выберите приложение.", LogLevel.Warn); return; }
            if (Shell.Install == null || !Shell.Install.IsValid) { Shell.Notify("Папка zapret не найдена.", LogLevel.Error); return; }
            if (_hostsEditor.Text.Trim().Length > 0 && !Dlg.Confirm(this.View.FindForm(), "Заменить список адресов?",
                "Текущий список в поле «Адреса для обхода» будет заменён результатами наблюдения.", "Продолжить")) return;

            AppEntry app = _apps[index];
            _appBusy = true;
            _btnAppWatch.Enabled = false;
            _btnAppRefresh.Enabled = false;
            _procStatus.Text = "Наблюдаем за " + app.Name + "…";
            SetButtons(false);

            Shell.RunBackground(delegate()
            {
                try
                {
                    List<string> ips;
                    List<string> hosts = AppProbe.Watch(app.Pid, 60, delegate(int percent, string stage)
                    {
                        Shell.Ui(delegate() { _procStatus.Text = stage; });
                    }, out ips);

                    List<string> okHosts;
                    List<string> blocked = AppProbe.BlockedHosts(hosts, delegate(int percent, string stage)
                    {
                        Shell.Ui(delegate() { _procStatus.Text = stage; });
                    }, out okHosts);

                    Shell.Ui(delegate()
                    {
                        List<string> lines = new List<string>();
                        lines.AddRange(blocked);
                        lines.AddRange(okHosts);
                        _hostsEditor.Text = string.Join("\r\n", lines.ToArray());
                        if (blocked.Count > 0)
                        {
                            _procStatus.Text = string.Format("{0}: доменов {1}, похоже на блокировку {2}. Нажмите «В мой список».", app.Name, hosts.Count, blocked.Count);
                            _procStatus.ForeColor = Theme.Warn;
                        }
                        else
                        {
                            _procStatus.Text = string.Format("{0}: доменов {1}, заблокированных не нашли. Адреса добавлены в список ниже для ручной проверки.", app.Name, hosts.Count);
                            _procStatus.ForeColor = Theme.TextMuted;
                        }
                        if (ips.Count > 0) _status.Text = "IP-адресов в соединениях: " + ips.Count + " (если домены не помогли — посмотрите «Проверить адреса»).";
                        SetButtons(lines.Count > 0);
                        Shell.Notify(blocked.Count > 0 ? "Найдены заблокированные домены: " + blocked.Count : "Заблокированных доменов не найдено.", blocked.Count > 0 ? LogLevel.Warn : LogLevel.Ok);
                    });
                }
                catch (Exception ex)
                {
                    Shell.Ui(delegate()
                    {
                        _procStatus.Text = "Ошибка наблюдения: " + ex.Message;
                        _procStatus.ForeColor = Theme.Danger;
                    });
                }
                finally
                {
                    _appBusy = false;
                    Shell.Ui(delegate() { _btnAppWatch.Enabled = true; _btnAppRefresh.Enabled = true; });
                }
            }, null);
        }
        private void BuildHostsCard()
        {
            Card card = NewCard("Адреса для обхода", 372);

            _hostsHint = WrapLabel("Здесь появляются домены сайта и связанные с ним адреса. Список можно править руками: одна строка — один домен. Поддомены указывать не обязательно, zapret сам учитывает их.",
                Theme.Small, Theme.TextDim, 22, 40, DesignWidth - 60);
            card.Controls.Add(_hostsHint);

            _hostsEditor = new DarkEditor();
            _hostsEditor.Location = new Point(22, 96);
            _hostsEditor.Size = new Size(DesignWidth - 44, 120);
            _hostsEditor.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right;
            card.Controls.Add(_hostsEditor);

            _btnProbeHosts = Btn("Проверить адреса", ButtonKind.Secondary, 170);
            _btnProbeHosts.Location = new Point(22, 232);
            _btnProbeHosts.Enabled = false;
            _btnProbeHosts.Click += delegate { ProbeList(); };
            card.Controls.Add(_btnProbeHosts);

            _btnToList = Btn("В мой список", ButtonKind.Primary, 160);
            _btnToList.Location = new Point(200, 232);
            _btnToList.Enabled = false;
            _btnToList.Click += delegate { AddToList(); };
            card.Controls.Add(_btnToList);

            _btnToHosts = Btn("Прописать в hosts", ButtonKind.Secondary, 178);
            _btnToHosts.Location = new Point(368, 232);
            _btnToHosts.Enabled = false;
            _btnToHosts.Click += delegate { AddToHosts(); };
            card.Controls.Add(_btnToHosts);

            _btnCopy = Btn("Скопировать", ButtonKind.Ghost, 130);
            _btnCopy.Location = new Point(554, 232);
            _btnCopy.Enabled = false;
            _btnCopy.Click += delegate
            {
                try
                {
                    Clipboard.SetText(Text());
                    Shell.Notify("Адреса скопированы в буфер обмена.", LogLevel.Ok);
                }
                catch (Exception ex) { Shell.Notify("Не удалось скопировать: " + ex.Message, LogLevel.Error); }
            };
            card.Controls.Add(_btnCopy);

            _status = L("", Theme.Small, Theme.TextMuted, 22, 274);
            card.Controls.Add(_status);

            card.Controls.Add(WrapLabel("Подсказка: если сайт открывается, но контент не грузится (как у Instagram), добавьте в список его CDN-домены — их видно по кнопке «Проверить адреса».",
                Theme.Small, Theme.TextMuted, 22, 300, DesignWidth - 60));
            View.Stack.Add(card);
        }

        private bool _suppressGuard;

        private void BuildGuardCard()
        {
            Card card = NewCard("Фоновый сторож", 278);
            card.Controls.Add(L("Следить за блокировками в фоне", Theme.Base, Theme.Text, 22, 44));
            card.Controls.Add(WrapLabel("Пока менеджер работает, он читает журнал DNS Windows: когда браузер запрашивает домен, сторож быстро проверяет его и добавляет в списки те, что рвутся из-за ТСПУ.",
                Theme.Small, Theme.TextDim, 22, 74, DesignWidth - 230));

            _tgGuard = new ToggleSwitch();
            _tgGuard.Anchor = AnchorStyles.Top | AnchorStyles.Right;
            _tgGuard.Location = new Point(DesignWidth - 22 - 46, 42);
            _tgGuard.CheckedChanged += delegate
            {
                if (_suppressGuard) return;
                Shell.Config.SetBool("SiteGuard", _tgGuard.Checked);
                Shell.Config.Save();
                if (_tgGuard.Checked)
                {
                    string message = SiteGuard.EnableLog();
                    SiteGuard.ResetCursor();
                    Shell.Notify(message, message.StartsWith("Не удалось") ? LogLevel.Error : LogLevel.Ok);
                }
                else
                {
                    Shell.Notify("Сторож выключен.", LogLevel.Ok);
                }
                RefreshGuard();
            };
            card.Controls.Add(_tgGuard);

            _guardStatus = L("", Theme.Small, Theme.TextMuted, 22, 128);
            card.Controls.Add(_guardStatus);

            card.Controls.Add(L("Добавлять найденное автоматически", Theme.Small, Theme.TextDim, 22, 164));
            _tgGuardAuto = new ToggleSwitch();
            _tgGuardAuto.Anchor = AnchorStyles.Top | AnchorStyles.Right;
            _tgGuardAuto.Location = new Point(DesignWidth - 22 - 46, 162);
            _tgGuardAuto.CheckedChanged += delegate
            {
                if (_suppressGuard) return;
                Shell.Config.SetBool("SiteGuardAutoAdd", _tgGuardAuto.Checked);
                Shell.Config.Save();
            };
            card.Controls.Add(_tgGuardAuto);

            RoundButton enable = Btn("Включить журнал DNS", ButtonKind.Secondary, 210);
            enable.Location = new Point(22, 196);
            enable.Click += delegate
            {
                string message = SiteGuard.EnableLog();
                Shell.Notify(message, message.StartsWith("Не удалось") ? LogLevel.Error : LogLevel.Ok);
                RefreshGuard();
            };
            card.Controls.Add(enable);

            card.Controls.Add(WrapLabel("Если в браузере включён DoH (DNS через HTTPS), запросы идут мимо Windows и сторож их не увидит — тогда проверяйте сайт вручную выше.",
                Theme.Small, Theme.TextMuted, 22, 238, DesignWidth - 60));

            View.Stack.Add(card);
        }

        private void RefreshGuard()
        {
            if (_tgGuard == null) return;
            _suppressGuard = true;
            try
            {
                _tgGuard.Checked = Shell.Config.GetBool("SiteGuard", false);
                _tgGuardAuto.Checked = Shell.Config.GetBool("SiteGuardAutoAdd", false);
                bool logOn = SiteGuard.LogEnabled();
                if (!_tgGuard.Checked) _guardStatus.Text = "Сторож выключен.";
                else if (!logOn) _guardStatus.Text = "Журнал DNS выключен — сторож не увидит запросы.";
                else _guardStatus.Text = "Сторож работает: журнал DNS включён, запросы браузеров под наблюдением.";
                _guardStatus.ForeColor = _tgGuard.Checked && !logOn ? Theme.Warn : Theme.TextMuted;
            }
            finally { _suppressGuard = false; }
        }

        private void BuildHelpCard()
        {
            Card card = NewCard("Как это работает", 168);
            card.Controls.Add(WrapLabel("Проверка идёт по этапам: DNS → TCP-соединение → TLS-рукопожатие с нужным SNI → HTTP-запрос. Если DNS отвечает, а соединение или TLS рвётся (reset/таймаут) — это подпись ТСПУ, и такой домен имеет смысл добавить в списки обхода.\n\nЕсли же рвётся и HTTP, и страница не открывается — виноват не только SNI: тогда помогает запись «IP + домен» в hosts (менеджер делает резервную копию файла и пишет только отсутствующие записи).",
                Theme.Small, Theme.TextDim, 22, 40, DesignWidth - 60));
            card.AccentStripe = Theme.TextMuted;
            View.Stack.Add(card);
        }

        private string Text()
        {
            return _hostsEditor.Text.Replace("\r\n", "\n").Trim();
        }

        private void RunCheck()
        {
            if (_working) return;
            string input = (_input.Text ?? "").Trim();
            if (input.Length == 0 || input == "https://")
            {
                Shell.Notify("Введите домен или ссылку, например instagram.com", LogLevel.Warn);
                return;
            }

            _working = true;
            _btnCheck.Enabled = false;
            _progress.Visible = true;
            _progress.Value = 5;
            SetVerdict("Проверяем " + input + "…", Theme.Text);
            _steps.Text = "";
            _status.Text = "";
            SetButtons(false);

            Shell.RunBackground(delegate()
            {
                SiteProbe probe = null;
                string error = "";
                try
                {
                    probe = SiteDoctor.Check(input, delegate(string stage)
                    {
                        Shell.Ui(delegate() { SetVerdict(stage, Theme.TextDim); });
                    });
                    if (probe.Verdict != "dns" && probe.Verdict != "down")
                    {
                        Shell.Ui(delegate()
                        {
                            _progress.Value = 55;
                            SetVerdict("Ищем связанные адреса…", Theme.TextDim);
                        });
                        probe.Related = SiteDoctor.Discover(input, probe, delegate(string stage)
                        {
                            Shell.Ui(delegate() { SetVerdict(stage, Theme.TextDim); });
                        });
                    }
                }
                catch (Exception ex) { error = ex.Message; }

                Shell.Ui(delegate()
                {
                    _working = false;
                    _btnCheck.Enabled = true;
                    _progress.Value = 100;
                    _progress.Visible = false;

                    if (probe == null)
                    {
                        SetVerdict("Ошибка проверки: " + error, Theme.Warn);
                        return;
                    }

                    _probe = probe;
                    SetVerdict(probe.VerdictText, probe.Verdict == "ok" ? Theme.Success : (probe.Verdict == "dpi" ? Theme.Warn : Theme.Danger));
                    _steps.Text = StepsText(probe);

                    List<string> hosts = new List<string>();
                    hosts.AddRange(probe.Related);
                    if (hosts.Count == 0 && probe.Host.Length > 0) hosts.Add(probe.Host);
                    _hostsEditor.Text = string.Join("\r\n", hosts.ToArray());
                    _status.Text = string.Format("Проверено {0}. Время: TLS {1} мс, TCP {2} мс.",
                        DateTime.Now.ToString("HH:mm:ss"), probe.TlsOk ? probe.TlsMs : -1, probe.TcpMs);
                    _status.ForeColor = Theme.TextMuted;
                    SetButtons(hosts.Count > 0);
                    Ready = true;
                    Shell.Notify(probe.Verdict == "dpi" ? "Похоже на блокировку ТСПУ — адреса готовы к добавлению." : "Проверка завершена.",
                        probe.Verdict == "dpi" ? LogLevel.Warn : LogLevel.Ok);
                });
            }, null);
        }

        private void SetVerdict(string text, Color color)
        {
            _verdict.Text = text;
            _verdict.ForeColor = color;
            _verdict.Height = TextRenderer.MeasureText(text, _verdict.Font, new Size(Math.Max(100, _verdict.Width), 2000), TextFormatFlags.WordBreak).Height + 4;
        }

        private static string StepsText(SiteProbe p)
        {
            StringBuilder sb = new StringBuilder();
            sb.AppendLine(p.DnsOk
                ? string.Format("DNS: {0}  ({1} шт.)", p.Ips.Length > 0 ? p.Ips[0] : "ок", p.Ips.Length)
                : "DNS: ошибка — " + p.DnsError);
            sb.AppendLine(p.DnsOk
                ? (p.TcpOk ? string.Format("TCP :443 — ок, {0} мс", p.TcpMs) : "TCP :443 — ошибка: " + p.TcpError)
                : "TCP :443 — пропущено");
            sb.AppendLine(p.TcpOk
                ? (p.TlsOk ? string.Format("TLS (SNI {0}) — ок, {1} мс", p.Host, p.TlsMs) : "TLS — ошибка: " + p.TlsError)
                : "TLS — пропущено");
            if (p.TlsOk)
            {
                sb.Append(p.HttpOk
                    ? string.Format("HTTP — {0}{1}", p.HttpStatus, p.HtmlTitle.Length > 0 ? ", заголовок: «" + Short(p.HtmlTitle, 60) + "»" : "")
                    : "HTTP — ошибка: " + p.HttpError);
            }
            return sb.ToString();
        }

        private static string Short(string text, int max)
        {
            if (string.IsNullOrEmpty(text)) return "";
            return text.Length <= max ? text : text.Substring(0, max - 1) + "…";
        }

        private void SetButtons(bool enabled)
        {
            _btnProbeHosts.Enabled = enabled;
            _btnToList.Enabled = enabled;
            _btnToHosts.Enabled = enabled;
            _btnCopy.Enabled = enabled;
        }

        private List<string> Hosts()
        {
            List<string> list = new List<string>();
            foreach (string line in Text().Split('\n'))
            {
                string t = line.Trim();
                if (t.Length == 0) continue;
                if (t.StartsWith("#")) continue;
                // «Проверить адреса» дописывает аннотации вида "host   # TLS: ...";
                // в список должен попасть только сам адрес.
                int hash = t.IndexOf('#');
                if (hash > 0) t = t.Substring(0, hash).Trim();
                if (t.Length == 0) continue;
                list.Add(t);
            }
            return list;
        }

        private void ProbeList()
        {
            List<string> hosts = Hosts();
            if (hosts.Count == 0) { Shell.Notify("Список пуст.", LogLevel.Warn); return; }
            _btnProbeHosts.Enabled = false;
            _status.Text = "Проверяем адреса…";

            Shell.RunBackground(delegate()
            {
                int ok = 0;
                int broken = 0;
                List<string> lines = new List<string>();
                foreach (string host in hosts)
                {
                    int ms;
                    string err;
                    bool tcp = SiteDoctor.TcpConnect(host, 443, 2500, out ms, out err);
                    bool tls = false;
                    string tlsError = "";
                    if (tcp)
                    {
                        int tlsMs;
                        tls = SiteDoctor.TlsHandshake(host, 4000, out tlsMs, out tlsError);
                    }
                    if (tls)
                    {
                        ok++;
                        lines.Add(host);
                    }
                    else
                    {
                        broken++;
                        lines.Add(host + "     # " + (tcp ? "TLS: " + tlsError : "TCP: " + err));
                    }
                }
                Shell.Ui(delegate()
                {
                    _btnProbeHosts.Enabled = true;
                    _hostsEditor.Text = string.Join("\r\n", lines.ToArray());
                    _status.Text = string.Format("Отвечают: {0}, рвутся: {1}. Строки с «#» — проблемы при проверке.", ok, broken);
                    _status.ForeColor = broken > 0 ? Theme.Warn : Theme.Success;
                    Shell.Notify(string.Format("Проверка адресов: отвечают {0}, рвутся {1}.", ok, broken), broken > 0 ? LogLevel.Warn : LogLevel.Ok);
                });
            }, null);
        }

        private void AddToList()
        {
            if (Shell.Install == null || !Shell.Install.IsValid)
            {
                Shell.Notify("Папка zapret не найдена.", LogLevel.Error);
                return;
            }
            List<string> hosts = Hosts();
            if (hosts.Count == 0) { Shell.Notify("Список пуст.", LogLevel.Warn); return; }

            int added;
            string path;
            string message = SiteDoctor.AddToUserList(Shell.Install, hosts, out added, out path);
            Log.Info("Проверка сайта: " + message + " → " + path);
            Shell.Notify(message, added > 0 ? LogLevel.Ok : LogLevel.Warn);
            if (added > 0)
            {
                _status.Text = message + " Файл: " + path;
                _status.ForeColor = Theme.Success;
                Shell.RefreshState(true);
                Shell.Notify("Если bypass запущен, перезапустите его, чтобы новые домены подхватились.", LogLevel.Warn);
            }
        }

        private void AddToHosts()
        {
            List<string> hosts = Hosts();
            if (hosts.Count == 0) { Shell.Notify("Список пуст.", LogLevel.Warn); return; }
            if (!Shell.RequireAdmin("Запись в hosts")) return;
            if (!Dlg.Confirm(this.View.FindForm(), "Прописать IP в hosts?",
                "Менеджер определит IP для этих доменов и допишет их в системный файл hosts (с резервной копией).\n\nЭто помогает, когда провайдер подменяет адреса или сайт не открывается без привязки IP. Для CDN-сайтов может, наоборот, замедлить загрузку — при проблемах просто верните копию файла.",
                "Записать")) return;

            int added = 0;
            string backup = "";
            Shell.RunOperation(delegate() { return SiteDoctor.AddToHosts(hosts, out added, out backup); }, delegate(string message)
            {
                Log.Info("Проверка сайта: " + message + (backup.Length > 0 ? " (копия: " + backup + ")" : ""));
                Shell.Notify(message, added > 0 ? LogLevel.Ok : LogLevel.Warn);
                _status.Text = message + (backup.Length > 0 ? "  Резервная копия: " + backup : "");
                _status.ForeColor = added > 0 ? Theme.Success : Theme.TextMuted;
            });
        }
        public override void RefreshState()
        {
            RefreshGuard();
            if (!_appBusy && _apps.Count == 0 && _procStatus != null) LoadApps();

            if (_working) return;
            if (Shell.Install == null || !Shell.Install.IsValid)
            {
                if (_btnToList != null) _btnToList.Enabled = false;
            }
        }
    }
}
