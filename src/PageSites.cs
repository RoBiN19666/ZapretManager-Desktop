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
            BuildHostsCard();
            BuildHelpCard();

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

            int added;
            string backup;
            string message = SiteDoctor.AddToHosts(hosts, out added, out backup);
            Log.Info("Проверка сайта: " + message + (backup.Length > 0 ? " (копия: " + backup + ")" : ""));
            Shell.Notify(message, added > 0 ? LogLevel.Ok : LogLevel.Warn);
            _status.Text = message + (backup.Length > 0 ? "  Резервная копия: " + backup : "");
            _status.ForeColor = added > 0 ? Theme.Success : Theme.TextMuted;
        }

        public override void RefreshState()
        {
            if (_working) return;
            if (Shell.Install == null || !Shell.Install.IsValid)
            {
                if (_btnToList != null) _btnToList.Enabled = false;
            }
        }
    }
}
