using System;
using System.Collections.Generic;
using System.Drawing;
using System.Text;
using System.Windows.Forms;

namespace ZapretManager
{
    public sealed class DiagnosticsPage : PageBase
    {
        private Card _runCard;
        private Card _resultCard;
        private RoundButton _btnRun;
        private RoundButton _btnCopy;
        private Label _summary;
        private Label _status;
        private List<CheckRow> _rows = new List<CheckRow>();
        private List<CheckResult> _results = new List<CheckResult>();
        private bool _running;

        public DiagnosticsPage(Shell shell) : base(shell) { }

        public override string Key { get { return "diagnostics"; } }
        public override string Title { get { return "Диагностика"; } }
        public override string Subtitle { get { return "Поиск конфликтов и типичных проблем с обходом"; } }
        public override string Icon { get { return "\uE9D2"; } }

        public override void Build()
        {
            _runCard = NewCard("Проверка системы", 148);
            _runCard.Controls.Add(WrapLabel("Проверяются службы Windows, конфликтующие программы, прокси, DNS, файл hosts, целостность списков и состояние драйвера WinDivert.",
                Theme.Small, Theme.TextMuted, 22, 40, DesignWidth - 60));

            _btnRun = Btn("Проверить систему", ButtonKind.Primary, 200);
            _btnRun.Location = new Point(22, 90);
            _btnRun.Click += delegate { RunChecks(); };
            _runCard.Controls.Add(_btnRun);

            _status = L("", Theme.Small, Theme.TextMuted, 234, 97);
            _runCard.Controls.Add(_status);

            _btnCopy = Btn("Скопировать отчёт", ButtonKind.Ghost, 170);
            _btnCopy.Anchor = AnchorStyles.Top | AnchorStyles.Right;
            _btnCopy.Location = new Point(DesignWidth - 22 - 170, 90);
            _btnCopy.Click += delegate { CopyReport(); };
            _runCard.Controls.Add(_btnCopy);
            View.Stack.Add(_runCard);

            _resultCard = NewCard("Результаты", 120);
            _summary = L("Проверка ещё не выполнялась.", Theme.Base, Theme.TextDim, 22, 44);
            _resultCard.Controls.Add(_summary);
            View.Stack.Add(_resultCard);
        }

        private void RunChecks()
        {
            if (_running) return;
            if (Shell.Install == null || !Shell.Install.IsValid)
            {
                Shell.Notify("Папка zapret не найдена — проверка отменена.", LogLevel.Error);
                return;
            }
            _running = true;
            _btnRun.Enabled = false;
            _status.Text = "Выполняется…";
            ClearRows();
            _summary.Text = "Выполняется проверка…";
            _resultCard.Height = 120;
            Shell.Form.SetStatusHint("Диагностика…");

            Shell.RunBackground(delegate()
            {
                try
                {
                    List<CheckResult> results = Diagnostics.Run(Shell.Install, Shell.Ops, Shell.Config, delegate(string step)
                    {
                        Shell.Ui(delegate() { _status.Text = step; });
                    });
                    Shell.Ui(delegate()
                    {
                        _running = false;
                        _btnRun.Enabled = true;
                        _status.Text = "Готово: " + DateTime.Now.ToString("HH:mm:ss");
                        Shell.Form.SetStatusHint("");
                        ShowResults(results);
                    });
                }
                finally
                {
                    Shell.Ui(delegate()
                    {
                        if (!_running) return;
                        _running = false;
                        _btnRun.Enabled = true;
                        _status.Text = "Проверка прервана — подробности в журнале.";
                        Shell.Form.SetStatusHint("");
                    });
                }
            }, null);
        }

        private void ClearRows()
        {
            foreach (CheckRow row in _rows) row.Dispose();
            _rows.Clear();
            _resultCard.Controls.Clear();
            _resultCard.Controls.Add(_summary);
        }

        private void ShowResults(List<CheckResult> results)
        {
            _results = results;
            ClearRows();
            int errors = 0, warns = 0, oks = 0;
            foreach (CheckResult r in results)
            {
                if (r.Severity == CheckSeverity.Error) errors++;
                else if (r.Severity == CheckSeverity.Warn) warns++;
                else oks++;
            }
            _summary.Text = string.Format("Ошибок: {0}   •   предупреждений: {1}   •   в порядке: {2}", errors, warns, oks);
            _summary.ForeColor = errors > 0 ? Theme.Danger : (warns > 0 ? Theme.Warn : Theme.Success);

            int y = 70;
            results.Sort(delegate(CheckResult a, CheckResult b) { return ((int)b.Severity).CompareTo((int)a.Severity); });
            foreach (CheckResult r in results)
            {
                CheckRow row = new CheckRow(r, OnAction);
                row.Location = new Point(20, y);
                row.Width = DesignWidth - 40;
                row.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right;
                _resultCard.Controls.Add(row);
                _rows.Add(row);
                y += row.Height;
            }
            _resultCard.Height = y + 16;
            View.Stack.PerformLayout();
            Log.Ok(string.Format("Диагностика завершена: ошибок {0}, предупреждений {1}", errors, warns));
        }

        private void OnAction(string actionId)
        {
            if (!Native.IsAdmin() && actionId != Diagnostics.ActionClearDiscord)
            {
                Shell.Notify("Для этого действия нужны права администратора.", LogLevel.Warn);
                return;
            }
            string title = null;
            string text = null;
            switch (actionId)
            {
                case Diagnostics.ActionRemoveWinDivert:
                    title = "Удалить драйвер WinDivert?";
                    text = "Службы WinDivert и WinDivert14 будут остановлены и удалены. Это может затронуть другие программы, использующие WinDivert.";
                    break;
                case Diagnostics.ActionRemoveConflicts:
                    title = "Удалить конфликтующие службы?";
                    text = "Будут остановлены и удалены службы GoodbyeDPI, discordfix_zapret, winws1 и winws2. Если вы ими пользуетесь — они перестанут запускаться.";
                    break;
                case Diagnostics.ActionClearDiscord:
                    title = "Очистить кэш Discord?";
                    text = "Discord будет закрыт, а папки Cache, Code Cache и GPUCache удалены.";
                    break;
                case Diagnostics.ActionCleanHosts:
                    title = "Убрать блок менеджера из hosts?";
                    text = "Из файла hosts будет удалён блок, добавленный Zapret Manager. Остальные записи останутся.";
                    break;
                case Diagnostics.ActionEnableTcp:
                    title = "Включить TCP timestamps?";
                    text = "Параметр netsh interface tcp set global timestamps=enabled будет включён для всей системы.";
                    break;
            }
            if (title != null && !Dlg.ConfirmDanger(this.View.FindForm(), title, text, "Выполнить")) return;

            Shell.RunOperation(delegate() { return Diagnostics.RunAction(actionId, Shell.Ops); }, delegate(string msg)
            {
                bool failed = msg.IndexOf("Не удалось", StringComparison.OrdinalIgnoreCase) >= 0
                    || msg.StartsWith("Ошибка", StringComparison.OrdinalIgnoreCase)
                    || msg.IndexOf("нужны права", StringComparison.OrdinalIgnoreCase) >= 0;
                Shell.Notify(msg, failed ? LogLevel.Error : LogLevel.Ok);
                RunChecks();
            });
        }

        private void CopyReport()
        {
            if (_results.Count == 0) { Shell.Notify("Сначала выполните проверку.", LogLevel.Warn); return; }
            StringBuilder sb = new StringBuilder();
            sb.AppendLine("Zapret Manager " + AppInfo.Version + " — отчёт диагностики " + DateTime.Now.ToString("dd.MM.yyyy HH:mm"));
            if (Shell.Install != null && Shell.Install.IsValid) sb.AppendLine("Установка: " + Shell.Install.Root + " (zapret " + Shell.Install.Version + ")");
            sb.AppendLine();
            foreach (CheckResult r in _results)
            {
                sb.AppendLine(string.Format("[{0}] {1}: {2}{3}", r.SeverityText, r.Name, r.Message, r.Hint.Length > 0 ? " | " + r.Hint : ""));
            }
            try
            {
                Clipboard.SetText(sb.ToString());
                Shell.Notify("Отчёт скопирован в буфер обмена.", LogLevel.Ok);
            }
            catch (Exception ex) { Shell.Notify("Не удалось скопировать: " + ex.Message, LogLevel.Error); }
        }

        public override void OnShow()
        {
            if (_results.Count == 0 && Shell.Install != null && Shell.Install.IsValid)
                _summary.Text = Shell.Install.Root + "  •  zapret " + Shell.Install.Version + "  •  нажмите «Проверить систему»";
        }

        public override void RefreshState()
        {
        }
    }
}
