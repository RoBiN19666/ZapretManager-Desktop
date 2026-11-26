using System;
using System.Drawing;
using System.IO;
using System.Windows.Forms;

namespace ZapretManager
{
    public sealed class ToolsPage : PageBase
    {
        private Card _hostsCard;
        private Card _serviceCard;
        private Card _testCard;
        private Card _discordCard;
        private Card _linksCard;
        private Card _aboutCard;

        private Label _hostsStatus;
        private Label _serviceStatus;
        private Label _discordStatus;

        public ToolsPage(Shell shell) : base(shell) { }

        public override string Key { get { return "tools"; } }
        public override string Title { get { return "Инструменты"; } }
        public override string Subtitle { get { return "Служба, hosts, тест стратегий и быстрый доступ к файлам"; } }
        public override string Icon { get { return "\uE90F"; } }

        public override void Build()
        {
            BuildServiceCard();
            BuildHostsCard();
            BuildTestCard();
            BuildDiscordCard();
            BuildLinksCard();
            BuildAboutCard();
        }

        private void BuildServiceCard()
        {
            _serviceCard = NewCard("Служба Windows", 178);
            _serviceStatus = L("", Theme.Base, Theme.TextDim, 22, 42);
            _serviceCard.Controls.Add(_serviceStatus);

            RoundButton install = Btn("Установить службу", ButtonKind.Secondary, 180);
            install.Location = new Point(22, 76);
            install.Click += delegate { InstallService(); };
            _serviceCard.Controls.Add(install);

            RoundButton remove = Btn("Удалить службу", ButtonKind.Danger, 160);
            remove.Location = new Point(212, 76);
            remove.Click += delegate { RemoveService(); };
            _serviceCard.Controls.Add(remove);

            _serviceCard.Controls.Add(WrapLabel("Служба запускает bypass автоматически при загрузке Windows. Пока служба активна, ручной запуск стратегий недоступен.",
                Theme.Small, Theme.TextMuted, 22, 118, DesignWidth - 60));
            View.Stack.Add(_serviceCard);
        }

        private void BuildHostsCard()
        {
            _hostsCard = NewCard("Файл hosts", 174);
            _hostsStatus = L("Нажмите «Проверить», чтобы сравнить hosts с официальным списком.", Theme.Base, Theme.TextDim, 22, 42);
            _hostsCard.Controls.Add(_hostsStatus);

            RoundButton check = Btn("Проверить", ButtonKind.Secondary, 120);
            check.Location = new Point(22, 80);
            check.Click += delegate { CheckHosts(); };
            _hostsCard.Controls.Add(check);

            RoundButton apply = Btn("Применить список", ButtonKind.Primary, 170);
            apply.Location = new Point(152, 80);
            apply.Click += delegate { ApplyHosts(); };
            _hostsCard.Controls.Add(apply);

            RoundButton clean = Btn("Убрать блок менеджера", ButtonKind.Ghost, 200);
            clean.Location = new Point(332, 80);
            clean.Click += delegate { CleanHosts(); };
            _hostsCard.Controls.Add(clean);

            _hostsCard.Controls.Add(WrapLabel("Записи добавляются в отдельный блок с маркерами, поэтому их легко удалить. Оригинальный файл сохраняется рядом как hosts.zapret-manager.bak.",
                Theme.Small, Theme.TextMuted, 22, 124, DesignWidth - 60));
            View.Stack.Add(_hostsCard);
        }

        private void BuildTestCard()
        {
            _testCard = NewCard("Проверка работоспособности", 150);
            _testCard.Controls.Add(WrapLabel("Официальный скрипт авторов по очереди запускает стратегии и проверяет доступность Discord, YouTube, Google и Cloudflare. Может занять несколько минут.",
                Theme.Small, Theme.TextMuted, 22, 40, DesignWidth - 60));

            RoundButton run = Btn("Запустить официальный тест", ButtonKind.Secondary, 250);
            run.Location = new Point(22, 92);
            run.Click += delegate { RunTests(); };
            _testCard.Controls.Add(run);
            View.Stack.Add(_testCard);
        }

        private void BuildDiscordCard()
        {
            _discordCard = NewCard("Кэш Discord", 170);
            _discordStatus = L("", Theme.Small, Theme.TextMuted, 22, 46);
            _discordCard.Controls.Add(_discordStatus);

            RoundButton clear = Btn("Очистить кэш Discord", ButtonKind.Secondary, 190);
            clear.Location = new Point(22, 76);
            clear.Click += delegate
            {
                if (!Dlg.Confirm(this.View.FindForm(), "Очистить кэш Discord?", "Discord будет закрыт, папки Cache, Code Cache и GPUCache будут удалены.", "Очистить")) return;
                string msg = Shell.Ops.ClearDiscordCache();
                Shell.Notify(msg, LogLevel.Ok);
                RefreshState();
            };
            _discordCard.Controls.Add(clear);

            _discordCard.Controls.Add(WrapLabel("Помогает, если после включения обхода Discord показывает бесконечную загрузку или не подключается к голосовому каналу.",
                Theme.Small, Theme.TextMuted, 22, 116, DesignWidth - 60));
            View.Stack.Add(_discordCard);
        }

        private void BuildLinksCard()
        {
            _linksCard = NewCard("Папки", 138);

            RoundButton b1 = Btn("Папка zapret", ButtonKind.Secondary, 150);
            b1.Location = new Point(22, 44);
            b1.Click += delegate { if (Shell.Install != null) Native.OpenFolder(Shell.Install.Root); };
            _linksCard.Controls.Add(b1);

            RoundButton b2 = Btn("bin", ButtonKind.Secondary, 100);
            b2.Location = new Point(182, 44);
            b2.Click += delegate { if (Shell.Install != null) Native.OpenFolder(Shell.Install.BinDir); };
            _linksCard.Controls.Add(b2);

            RoundButton b3 = Btn("lists", ButtonKind.Secondary, 100);
            b3.Location = new Point(292, 44);
            b3.Click += delegate { if (Shell.Install != null) Native.OpenFolder(Shell.Install.ListsDir); };
            _linksCard.Controls.Add(b3);

            RoundButton b4 = Btn("utils", ButtonKind.Secondary, 100);
            b4.Location = new Point(402, 44);
            b4.Click += delegate { if (Shell.Install != null) Native.OpenFolder(Shell.Install.UtilsDir); };
            _linksCard.Controls.Add(b4);

            RoundButton tgDir = Btn("Папка TG WS Proxy", ButtonKind.Secondary, 190);
            tgDir.Location = new Point(22, 88);
            tgDir.Click += delegate
            {
                TgProxyState st = TgProxy.GetState(Shell.Config);
                Native.OpenFolder(st.Dir.Length > 0 ? st.Dir : TgProxy.DefaultDir);
            };
            _linksCard.Controls.Add(tgDir);

            RoundButton wizard = Btn("Мастер установки", ButtonKind.Secondary, 180);
            wizard.Location = new Point(222, 88);
            wizard.Click += delegate
            {
                using (SetupWizard w = new SetupWizard(Shell)) { w.ShowDialog(this.View.FindForm()); }
                Shell.RefreshState(true);
            };
            _linksCard.Controls.Add(wizard);

            RoundButton journal = Btn("Журнал менеджера", ButtonKind.Secondary, 180);
            journal.Location = new Point(412, 88);
            journal.Click += delegate { Native.SelectInExplorer(Path.Combine(AppPaths.DataDir, "manager.log")); };
            _linksCard.Controls.Add(journal);

            View.Stack.Add(_linksCard);
        }
        private void BuildAboutCard()
        {
            _aboutCard = NewCard("О программе", 120);

            Panel logoBox = new Panel();
            logoBox.Location = new Point(DesignWidth - 356, 34);
            logoBox.Size = new Size(334, 126);
            logoBox.Anchor = AnchorStyles.Top | AnchorStyles.Right;
            logoBox.BackColor = Color.Transparent;
            logoBox.Paint += delegate(object s, PaintEventArgs e)
            {
                e.Graphics.SmoothingMode = System.Drawing.Drawing2D.SmoothingMode.AntiAlias;
                Brand.DrawLogo(e.Graphics, new Rectangle(0, 0, logoBox.Width, logoBox.Height), ContentAlignment.MiddleCenter);
            };
            _aboutCard.Controls.Add(logoBox);

            Label aboutText = WrapLabel(string.Format("Zapret Manager {0} — неофициальная графическая оболочка.\nОбход DPI — проект zapret (bol-van), сборка и стратегии — zapret-discord-youtube (Flowseal), прокси Telegram — TG WS Proxy (Flowseal). Сам обход выполняют winws.exe и драйвер WinDivert из состава zapret.",
                AppInfo.Version), Theme.Small, Theme.TextDim, 22, 42, DesignWidth - 396);
            _aboutCard.Controls.Add(aboutText);

            int y = aboutText.Bottom + 8;

            RoundButton repo = Btn("ZapretManager", ButtonKind.Link, 160);
            repo.Location = new Point(22, y);
            repo.Click += delegate { Native.OpenUrl("https://github.com/RoBiN19666/ZapretManager-Windows"); };
            _aboutCard.Controls.Add(repo);

            RoundButton zapret = Btn("zapret-discord-youtube", ButtonKind.Link, 210);
            zapret.Location = new Point(200, y);
            zapret.Click += delegate { Native.OpenUrl("https://github.com/Flowseal/zapret-discord-youtube"); };
            _aboutCard.Controls.Add(zapret);

            RoundButton bolvan = Btn("zapret-win-bundle (bol-van)", ButtonKind.Link, 230);
            bolvan.Location = new Point(22, y + 30);
            bolvan.Click += delegate { Native.OpenUrl("https://github.com/bol-van/zapret-win-bundle"); };
            _aboutCard.Controls.Add(bolvan);

            RoundButton core = Btn("исходный zapret", ButtonKind.Link, 150);
            core.Location = new Point(262, y + 30);
            core.Click += delegate { Native.OpenUrl("https://github.com/bol-van/zapret"); };
            _aboutCard.Controls.Add(core);

            RoundButton guide = Btn("Документация", ButtonKind.Link, 150);
            guide.Location = new Point(200, y + 60);
            guide.Click += delegate { Native.OpenUrl("https://github.com/RoBiN19666/ZapretManager-Windows/blob/main/docs/guide.md"); };
            _aboutCard.Controls.Add(guide);

            RoundButton tgproxy = Btn("TG WS Proxy", ButtonKind.Link, 130);
            tgproxy.Location = new Point(22, y + 60);
            tgproxy.Click += delegate { Native.OpenUrl("https://github.com/Flowseal/tg-ws-proxy"); };
            _aboutCard.Controls.Add(tgproxy);
            Label v = L("Настройки и журнал: " + AppPaths.DataDir, Theme.Small, Theme.TextMuted, 22, y + 96);
            _aboutCard.Controls.Add(v);

            _aboutCard.Height = y + 96 + 16 + 14;
            View.Stack.Add(_aboutCard);
        }
        private void InstallService()
        {
            if (Shell.Install == null || !Shell.Install.IsValid) { Shell.Notify("Папка zapret не найдена.", LogLevel.Error); return; }
            if (!Shell.RequireAdmin("Установка службы")) return;
            StrategyDef s = StrategyPicker.Pick(Shell);
            if (s == null) { Shell.Notify("Стратегии не найдены.", LogLevel.Error); return; }
            if (!Dlg.Confirm(this.View.FindForm(), "Установить службу?",
                string.Format("Стратегия «{0}» будет установлена как служба «zapret» с автозапуском.", s.Name), "Установить")) return;
            bool ok;
            string msg = Shell.Ops.InstallService(s, out ok);
            Shell.Notify(msg, ok ? LogLevel.Ok : LogLevel.Error);
            Shell.RefreshState(false);
            RefreshState();
            if (!ok) Dlg.Info(this.View.FindForm(), "Не удалось установить службу", msg);
        }

        private void RemoveService()
        {
            if (!Shell.RequireAdmin("Удаление службы")) return;
            if (!Dlg.ConfirmDanger(this.View.FindForm(), "Удалить службу?",
                "Служба zapret будет остановлена и удалена, драйвер WinDivert освобождён. Bypass перестанет работать, пока вы снова не запустите его вручную.",
                "Удалить службу")) return;
            string msg = Shell.Ops.RemoveService();
            Shell.Notify(msg, LogLevel.Ok);
            Shell.RefreshState(false);
            RefreshState();
        }

        private void CheckHosts()
        {
            _hostsStatus.Text = "Проверка hosts…";
            Shell.RunBackground(delegate()
            {
                ZapretOps.HostsStatus st = Shell.Ops.CheckHosts();
                Shell.Ui(delegate()
                {
                    if (!st.Downloaded) _hostsStatus.Text = "Не удалось получить список: " + st.Error;
                    else if (st.MissingLines == 0) _hostsStatus.Text = string.Format("hosts актуален: все {0} записей на месте.", st.RemoteLines);
                    else _hostsStatus.Text = string.Format("В hosts не хватает {0} из {1} записей — нажмите «Применить список».", st.MissingLines, st.RemoteLines);
                });
            }, null);
        }

        private void ApplyHosts()
        {
            if (!Shell.RequireAdmin("Изменение hosts")) return;
            if (!Dlg.Confirm(this.View.FindForm(), "Обновить hosts?",
                "В файл hosts будут добавлены записи из официального репозитория. Оригинал сохранится как hosts.zapret-manager.bak.", "Применить")) return;
            _hostsStatus.Text = "Обновление hosts…";
            Shell.RunBackground(delegate()
            {
                string msg = Shell.Ops.ApplyHosts();
                Shell.Ui(delegate()
                {
                    _hostsStatus.Text = msg;
                    Shell.Notify(msg, msg.StartsWith("Ошибка") || msg.StartsWith("Не удалось") ? LogLevel.Error : LogLevel.Ok);
                });
            }, null);
        }

        private void CleanHosts()
        {
            if (!Shell.RequireAdmin("Изменение hosts")) return;
            if (!Dlg.ConfirmDanger(this.View.FindForm(), "Убрать блок менеджера?",
                "Из файла hosts будет удалён блок, добавленный Zapret Manager. Остальные записи останутся.", "Удалить")) return;
            string msg = Shell.Ops.ClearHostsBlock();
            _hostsStatus.Text = msg;
            Shell.Notify(msg, LogLevel.Ok);
        }

        private void RunTests()
        {
            if (Shell.Install == null || !Shell.Install.IsValid) return;
            string script = Path.Combine(Shell.Install.UtilsDir, "test zapret.ps1");
            if (!File.Exists(script)) { Shell.Notify("Не найден файл utils\\test zapret.ps1", LogLevel.Error); return; }
            try
            {
                System.Diagnostics.ProcessStartInfo psi = new System.Diagnostics.ProcessStartInfo("powershell.exe",
                    "-NoProfile -ExecutionPolicy Bypass -File \"" + script + "\"");
                psi.UseShellExecute = true;
                psi.WorkingDirectory = Shell.Install.Root;
                System.Diagnostics.Process.Start(psi);
                Shell.Notify("Тест запущен в отдельном окне PowerShell. Он останавливает текущий bypass и перебирает стратегии.", LogLevel.Info);
            }
            catch (Exception ex) { Shell.Notify("Не удалось запустить тест: " + ex.Message, LogLevel.Error); }
        }

        public override void OnShow()
        {
            RefreshState();
        }

        public override void RefreshState()
        {
            RuntimeState st = Shell.State;
            if (st != null)
            {
                _serviceStatus.Text = string.Format("Служба zapret: {0}{1}", st.ServiceInstalled ? st.ServiceStatus : "не установлена",
                    st.ServiceInstalled && st.ServiceStrategy.Length > 0 ? "  •  стратегия из реестра: " + st.ServiceStrategy : "");
                _serviceStatus.ForeColor = st.ServiceInstalled ? Theme.Text : Theme.TextDim;
            }
            System.Collections.Generic.List<string> dirs = ZapretOps.DiscordCacheDirs();
            _discordStatus.Text = dirs.Count == 0 ? "Установки Discord не найдены." : "Найдено: " + string.Join(", ", dirs.ToArray());
        }
    }
}
