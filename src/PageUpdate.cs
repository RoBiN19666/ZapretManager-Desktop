using System;
using System.Drawing;
using System.IO;
using System.Windows.Forms;

namespace ZapretManager
{
    public sealed class UpdatePage : PageBase
    {
        private Card _versionCard;
        private Card _updateCard;
        private Card _freshCard;
        private Card _optionsCard;

        private Label _installedTitle;
        private Label _installedPath;
        private Label _checkStatus;
        private RoundButton _btnCheck;

        private Label _relTitle;
        private Label _relDate;
        private DarkEditor _notes;
        private ToggleSwitch _tgBackup;
        private ToggleSwitch _tgRestart;
        private ToggleSwitch _tgLists;
        private RoundButton _btnInstall;
        private LineProgress _progress;
        private Label _stage;
        private bool _working;

        private ToggleSwitch _tgStartupCheck;
        private bool _suppress;

        public UpdatePage(Shell shell) : base(shell) { }

        public override string Key { get { return "update"; } }
        public override string Title { get { return "Обновление"; } }
        public override string Subtitle { get { return "Обновление zapret до новой версии с сохранением ваших списков"; } }
        public override string Icon { get { return "\uE896"; } }

        public override void Build()
        {
            BuildVersionCard();
            BuildUpdateCard();
            BuildFreshCard();
            BuildOptionsCard();
            Shell.UpdateStateChanged += RefreshState;
        }

        private void BuildVersionCard()
        {
            _versionCard = NewCard("Установленная версия", 148);
            _installedTitle = L("", Theme.H2, Theme.Text, 22, 40);
            _versionCard.Controls.Add(_installedTitle);
            _installedPath = L("", Theme.Small, Theme.TextMuted, 22, 68);
            _versionCard.Controls.Add(_installedPath);
            _checkStatus = L("", Theme.Small, Theme.TextDim, 22, 94);
            _versionCard.Controls.Add(_checkStatus);

            _btnCheck = Btn("Проверить обновления", ButtonKind.Secondary, 205);
            _btnCheck.Anchor = AnchorStyles.Top | AnchorStyles.Right;
            _btnCheck.Location = new Point(DesignWidth - 22 - 205, 54);
            _btnCheck.Click += delegate { CheckNow(); };
            _versionCard.Controls.Add(_btnCheck);
            View.Stack.Add(_versionCard);
        }

        private void BuildUpdateCard()
        {
            _updateCard = NewCard("Доступное обновление", 368);
            _relTitle = L("Проверяем…", Theme.H2, Theme.Text, 22, 40);
            _updateCard.Controls.Add(_relTitle);
            _relDate = L("", Theme.Small, Theme.TextMuted, 22, 66);
            _updateCard.Controls.Add(_relDate);

            _notes = new DarkEditor();
            _notes.ReadOnly = true;
            _notes.SetWrapped();
            _notes.Location = new Point(22, 90);
            _notes.Size = new Size(DesignWidth - 44, 118);
            _notes.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right;
            _updateCard.Controls.Add(_notes);

            Label lbBackup = L("Создать резервную копию", Theme.Small, Theme.TextDim, 22, 216);
            _updateCard.Controls.Add(lbBackup);
            _tgBackup = new ToggleSwitch();
            _tgBackup.Checked = true;
            _tgBackup.Location = new Point(lbBackup.Right + 10, 210);
            _updateCard.Controls.Add(_tgBackup);

            Label lbRestart = L("Перезапустить bypass после обновления", Theme.Small, Theme.TextDim, _tgBackup.Right + 34, 216);
            _updateCard.Controls.Add(lbRestart);
            _tgRestart = new ToggleSwitch();
            _tgRestart.Checked = true;
            _tgRestart.Location = new Point(lbRestart.Right + 10, 210);
            _updateCard.Controls.Add(_tgRestart);

            Label lbLists = L("Обновлять официальные списки", Theme.Small, Theme.TextDim, 22, 246);
            _updateCard.Controls.Add(lbLists);
            _tgLists = new ToggleSwitch();
            _tgLists.Checked = true;
            _tgLists.Location = new Point(lbLists.Right + 10, 240);
            _updateCard.Controls.Add(_tgLists);
            _updateCard.Controls.Add(L("выкл. — останутся ваши list-general / list-google / list-exclude", Theme.Small, Theme.TextMuted, _tgLists.Right + 14, 247));

            _btnInstall = Btn("Скачать и установить", ButtonKind.Primary, 210);
            _btnInstall.Location = new Point(22, 284);
            _btnInstall.Click += delegate { InstallUpdate(); };
            _updateCard.Controls.Add(_btnInstall);

            _progress = new LineProgress();
            _progress.Location = new Point(244, 294);
            _progress.Width = DesignWidth - 266;
            _progress.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right;
            _updateCard.Controls.Add(_progress);

            _stage = L("", Theme.Small, Theme.TextMuted, 22, 324);
            _updateCard.Controls.Add(_stage);
            View.Stack.Add(_updateCard);
        }

        private void BuildFreshCard()
        {
            _freshCard = NewCard("Установка с нуля", 132);
            _freshCard.Controls.Add(WrapLabel("Если zapret ещё не установлен: программа скачает официальный архив, распакует его в выбранную папку и сразу подхватит.",
                Theme.Small, Theme.TextMuted, 22, 40, DesignWidth - 60));
            RoundButton dl = Btn("Скачать и установить zapret", ButtonKind.Secondary, 230);
            dl.Location = new Point(22, 82);
            dl.Click += delegate { InstallFresh(); };
            _freshCard.Controls.Add(dl);
            RoundButton pick = Btn("Указать папку…", ButtonKind.Ghost, 150);
            pick.Location = new Point(264, 82);
            pick.Click += delegate { PickFolder(); };
            _freshCard.Controls.Add(pick);
            View.Stack.Add(_freshCard);
        }

        private void BuildOptionsCard()
        {
            _optionsCard = NewCard("Параметры", 116);
            _optionsCard.Controls.Add(L("Проверять обновления при запуске менеджера", Theme.Bold, Theme.Text, 22, 42));
            _optionsCard.Controls.Add(L("Запрос к GitHub при каждом старте программы", Theme.Small, Theme.TextMuted, 22, 61));
            _tgStartupCheck = new ToggleSwitch();
            _tgStartupCheck.Checked = Shell.Config.GetBool("CheckUpdatesOnStartup", true);
            _tgStartupCheck.Anchor = AnchorStyles.Top | AnchorStyles.Right;
            _tgStartupCheck.Location = new Point(DesignWidth - 22 - 46, 46);
            _tgStartupCheck.CheckedChanged += delegate
            {
                if (_suppress) return;
                Shell.Config.SetBool("CheckUpdatesOnStartup", _tgStartupCheck.Checked);
                Shell.Config.Save();
            };
            _optionsCard.Controls.Add(_tgStartupCheck);
            View.Stack.Add(_optionsCard);
        }

        private void PickFolder()
        {
            OpenFileDialog dlg = new OpenFileDialog();
            dlg.Title = "Выберите service.bat в папке zapret";
            dlg.Filter = "service.bat|service.bat|Все файлы (*.*)|*.*";
            if (dlg.ShowDialog() == DialogResult.OK)
            {
                string root = Path.GetDirectoryName(dlg.FileName);
                if (ZapretInstall.LooksValid(root)) Shell.SetRoot(root);
                else Shell.Notify("В выбранной папке нет service.bat и bin\\winws.exe", LogLevel.Error);
            }
        }

        private void CheckNow()
        {
            _btnCheck.Enabled = false;
            _checkStatus.Text = "Проверка обновлений…";
            string local = Shell.LocalVersion;
            Shell.RunBackground(delegate()
            {
                UpdateCheckResult res = UpdateChecker.Check(local, true);
                Shell.Ui(delegate()
                {
                    _btnCheck.Enabled = true;
                    Shell.UpdateState = res;
                    Shell.RaiseUpdateStateChanged();
                    if (!res.Ok) Shell.Notify("Не удалось проверить обновления: " + res.Error, LogLevel.Error);
                    else if (res.UpdateAvailable) Shell.Notify("Доступна версия " + res.RemoteVersion, LogLevel.Warn);
                    else Shell.Notify("Установлена последняя версия " + local + ".", LogLevel.Ok);
                    RefreshState();
                });
            }, null);
        }

        private void InstallUpdate()
        {
            if (_working) return;
            UpdateCheckResult st = Shell.UpdateState;
            if (st == null || !st.Ok || st.Release == null) { Shell.Notify("Сначала проверьте обновления.", LogLevel.Warn); return; }
            if (!Shell.Install.IsValid) { Shell.Notify("Папка zapret не найдена.", LogLevel.Error); return; }
            if (!Shell.RequireAdmin("Обновление")) return;
            if (!Dlg.Confirm(this.View.FindForm(), "Обновить zapret?",
                string.Format("Будет скачана и установлена версия {0} с официального репозитория Flowseal. Ваши пользовательские списки, IP-лист и настройки сохранятся.\n\nВо время обновления bypass будет остановлен.", st.RemoteVersion),
                "Обновить")) return;

            _working = true;
            _btnInstall.Enabled = false;
            _btnCheck.Enabled = false;
            _progress.Value = 0;
            _stage.Text = "Подготовка…";
            Shell.Form.SetStatusHint("Обновление zapret…");

            UpdateOptions opt = new UpdateOptions();
            opt.CreateBackup = _tgBackup.Checked;
            opt.RestartAfter = _tgRestart.Checked;
            opt.PreserveUserLists = true;
            opt.UpdateOfficialLists = _tgLists.Checked;
            ReleaseInfo rel = st.Release;

            Updater upd = new Updater(Shell.Install, Shell.Ops, delegate(UpdateProgress p)
            {
                Shell.Ui(delegate()
                {
                    _progress.Value = p.Percent;
                    if (p.Stage.Length > 0) _stage.Text = p.Stage;
                    if (p.Finished)
                    {
                        _working = false;
                        _btnInstall.Enabled = true;
                        _btnCheck.Enabled = true;
                        Shell.Form.SetStatusHint("");
                        if (p.Ok)
                        {
                            Shell.Notify(p.Summary.Replace("\n", " "), LogLevel.Ok);
                            _stage.Text = "Готово. " + p.Summary.Replace("\n", " ");
                            Shell.Install.Rescan();
                            Shell.Config.Set("LastUpdate", DateTime.Now.ToString("yyyy-MM-dd HH:mm"));
                            Shell.Config.Save();
                            Shell.RefreshState(true);
                            UpdateChecker.Last = null;
                        }
                        else
                        {
                            _stage.Text = "Ошибка: " + p.Error;
                            Shell.Notify("Ошибка обновления: " + p.Error, LogLevel.Error);
                            Dlg.Info(this.View.FindForm(), "Обновление не удалось", p.Error + (p.BackupCreated ? "\n\nРезервная копия: " + p.BackupPath : ""));
                        }
                        RefreshState();
                    }
                });
            });

            Shell.RunBackground(delegate()
            {
                upd.UpdateNow(rel, opt);
            }, null);
        }

        private void InstallFresh()
        {
            if (_working) return;
            if (!Shell.RequireAdmin("Установка zapret")) return;
            FolderBrowserDialog dlg = new FolderBrowserDialog();
            dlg.Description = "Куда установить zapret? Внутри будет создана папка zapret-discord-youtube-<версия>.";
            if (dlg.ShowDialog() != DialogResult.OK) return;

            _working = true;
            Shell.Form.SetStatusHint("Установка zapret…");
            _progress.Value = 0;
            string parent = dlg.SelectedPath;
            Shell.RunBackground(delegate()
            {
                try
                {
                    ReleaseInfo rel = GitHub.GetLatestRelease(GitHub.OfficialRepo);
                    ZapretInstall inst = Updater.InstallFresh(rel, parent, delegate(int p, string stage)
                    {
                        Shell.Ui(delegate()
                        {
                            _progress.Value = p;
                            _stage.Text = stage;
                        });
                    });
                    Shell.Ui(delegate()
                    {
                        _working = false;
                        Shell.SetRoot(inst.Root);
                        Shell.Notify("zapret " + rel.Tag + " установлен в " + inst.Root, LogLevel.Ok);
                        UpdateChecker.Last = null;
                        Shell.RefreshState(true);
                    });
                }
                catch (Exception ex)
                {
                    Shell.Ui(delegate()
                    {
                        _working = false;
                        Shell.Notify("Установка не удалась: " + ex.Message, LogLevel.Error);
                        Dlg.Info(this.View.FindForm(), "Установка не удалась", ex.Message);
                    });
                }
                Shell.Ui(delegate() { Shell.Form.SetStatusHint(""); });
            }, null);
        }

        public override void OnShow()
        {
            RefreshState();
        }

        public override void RefreshState()
        {
            _suppress = true;
            try
            {
                _tgStartupCheck.Checked = Shell.Config.GetBool("CheckUpdatesOnStartup", true);
                bool valid = Shell.Install != null && Shell.Install.IsValid;
                _installedTitle.Text = valid ? "zapret " + Shell.LocalVersion : "zapret не найден";
                _installedPath.Text = valid ? Shell.Install.Root : "Укажите папку zapret или установите его автоматически";
                if (_versionCard.Width != DesignWidth) _versionCard.Width = DesignWidth;
            }
            finally { _suppress = false; }

            UpdateCheckResult st = Shell.UpdateState;
            if (st == null)
            {
                _relTitle.Text = "Проверка не выполнялась";
                _relDate.Text = "Нажмите «Проверить обновления», чтобы узнать о новой версии.";
                _btnInstall.Enabled = false;
                _checkStatus.Text = "";
                return;
            }
            if (!st.Ok)
            {
                _relTitle.Text = "Не удалось проверить обновления";
                _relDate.Text = st.Error;
                _btnInstall.Enabled = false;
                _checkStatus.Text = "Проверено: " + st.CheckedAt.ToString("HH:mm:ss");
                return;
            }
            _checkStatus.Text = string.Format("Проверено в {0:HH:mm:ss}. Актуальная версия: {1}", st.CheckedAt, st.RemoteVersion);
            if (st.UpdateAvailable)
            {
                _relTitle.Text = string.Format("Доступна новая версия {0}", st.RemoteVersion);
                _relTitle.ForeColor = Theme.Warn;
                string published = st.Release.Published > new DateTime(2000, 1, 1) ? string.Format("Опубликована {0:dd.MM.yyyy HH:mm}. ", st.Release.Published) : "";
                _relDate.Text = published + "У вас " + st.LocalVersion + " (источник: " + (st.Source == "version.txt" ? "version.txt" : "GitHub API") + ").";
                string body = st.Release.CleanBody();
                if (body.Length == 0) body = "Описание изменений не указано.";
                if (_notes.Text != body) _notes.Text = body;
                _btnInstall.Enabled = Shell.Install.IsValid && !_working;
            }
            else
            {
                _relTitle.Text = "Установлена последняя версия";
                _relTitle.ForeColor = Theme.Success;
                _relDate.Text = string.Format("zapret {0} — актуальная версия на GitHub.", st.LocalVersion);
                _notes.Text = "";
                _btnInstall.Enabled = false;
            }
            _updateCard.Visible = true;
        }
    }
}
