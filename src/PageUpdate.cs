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

        private Card _selfCard;
        private Label _selfVersion;
        private Label _selfStatus;
        private RoundButton _btnSelfCheck;
        private RoundButton _btnSelfUpdate;
        private LineProgress _selfProgress;
        private bool _selfBusy;

        private DarkSelect _versionSelect;
        private Label _versionHint;
        private RoundButton _btnVersionLoad;
        private RoundButton _btnVersionInstall;
        private LineProgress _versionProgress;
        private System.Collections.Generic.List<ReleaseInfo> _releases;
        private bool _versionBusy;
        private bool _versionsTried;
        private InputBox _manualTag;
        private RoundButton _btnManualInstall;

        public UpdatePage(Shell shell) : base(shell) { }

        public override string Key { get { return "update"; } }
        public override string Title { get { return "Обновление"; } }
        public override string Subtitle { get { return "Обновление zapret до новой версии с сохранением ваших списков"; } }
        public override string Icon { get { return "\uE896"; } }

        public override void Build()
        {
            BuildVersionCard();
            BuildUpdateCard();
            BuildSelfCard();
            BuildVersionsCard();
            BuildFreshCard();
            BuildOptionsCard();
            Shell.UpdateStateChanged += RefreshState;
        }

        public override void OnDispose()
        {
            Shell.UpdateStateChanged -= RefreshState;
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

        private void BuildVersionsCard()
        {
            Card card = NewCard("Версия zapret (сборки Flowseal)", 224);
            card.Controls.Add(WrapLabel("Можно поставить не только последнюю, но и любую другую версию — например, откатиться назад, если в новой что-то сломалось. Списки и настройки сохраняются.",
                Theme.Small, Theme.TextDim, 22, 40, DesignWidth - 60));

            _versionSelect = new DarkSelect();
            _versionSelect.Location = new Point(22, 88);
            _versionSelect.Width = 300;
            card.Controls.Add(_versionSelect);

            _btnVersionLoad = Btn("Загрузить список", ButtonKind.Secondary, 170);
            _btnVersionLoad.Location = new Point(332, 86);
            _btnVersionLoad.Click += delegate { LoadVersions(); };
            card.Controls.Add(_btnVersionLoad);

            _btnVersionInstall = Btn("Установить версию", ButtonKind.Primary, 180);
            _btnVersionInstall.Location = new Point(512, 86);
            _btnVersionInstall.Enabled = false;
            _btnVersionInstall.Click += delegate { InstallChosenVersion(); };
            card.Controls.Add(_btnVersionInstall);

            card.Controls.Add(L("Или тег вручную:", Theme.Small, Theme.TextMuted, 22, 130));
            _manualTag = new InputBox();
            _manualTag.Location = new Point(150, 128);
            _manualTag.Width = 150;
            card.Controls.Add(_manualTag);

            _btnManualInstall = Btn("Поставить по тегу", ButtonKind.Secondary, 180);
            _btnManualInstall.Location = new Point(312, 126);
            _btnManualInstall.Click += delegate { InstallManualTag(); };
            card.Controls.Add(_btnManualInstall);

            _versionHint = L("Список версий ещё не загружен.", Theme.Small, Theme.TextMuted, 22, 168);
            card.Controls.Add(_versionHint);

            _versionProgress = new LineProgress();
            _versionProgress.Location = new Point(22, 192);
            _versionProgress.Width = DesignWidth - 44;
            _versionProgress.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right;
            _versionProgress.Visible = false;
            card.Controls.Add(_versionProgress);

            View.Stack.Add(card);
        }

        private void LoadVersions()
        {
            if (_versionBusy) return;
            _versionBusy = true;
            _versionBusy = false;
            _versionBusy = true;
            _btnVersionLoad.Enabled = false;
            _btnVersionInstall.Enabled = false;
            _versionHint.Text = "Загрузка списка версий…";
            _versionHint.ForeColor = Theme.TextMuted;

            Shell.RunBackground(delegate()
            {
                string error = "";
                System.Collections.Generic.List<ReleaseInfo> list = null;
                try { list = GitHub.ListReleases(GitHub.OfficialRepo, 20); }
                catch (Exception ex) { error = ex.Message; }

                Shell.Ui(delegate()
                {
                    _versionBusy = false;
                    _btnVersionLoad.Enabled = true;
                    if (list == null || list.Count == 0)
                    {
                        _versionHint.Text = error.Length > 0
                            ? "Не удалось загрузить список: " + error + ". GitHub ограничивает частоту запросов — попробуйте позже."
                            : "GitHub не вернул ни одного релиза.";
                        _versionHint.ForeColor = Theme.Warn;
                        return;
                    }
                    _releases = list;
                    System.Collections.Generic.List<string> items = new System.Collections.Generic.List<string>();
                    int selected = 0;
                    string local = Shell.LocalVersion;
                    for (int i = 0; i < list.Count; i++)
                    {
                        string tag = list[i].Tag;
                        bool current = string.Equals(Versions.NormalizeTag(tag), local, StringComparison.OrdinalIgnoreCase);
                        items.Add(current ? tag + "   •   установлена" : tag);
                        if (current) selected = i;
                    }
                    _versionSelect.SetItems(items, items[selected]);
                    _versionSelect.SelectedIndex = selected;
                    _versionHint.Text = string.Format("Доступно версий: {0}. Установлена {1}.", list.Count, local);
                    Log.Info(string.Format("Список версий Flowseal: {0} шт., установлена {1}, первая {2}.", list.Count, local, list[0].Tag));
                    _versionHint.ForeColor = Theme.TextMuted;
                    _btnVersionInstall.Enabled = true;
                });
            }, null);
        }

        private void InstallChosenVersion()
        {
            if (_versionBusy || _releases == null) return;
            int index = _versionSelect.SelectedIndex;
            if (index < 0 || index >= _releases.Count) { Shell.Notify("Выберите версию в списке.", LogLevel.Warn); return; }
            InstallRelease(_releases[index]);
        }

        private void InstallManualTag()
        {
            string tag = (_manualTag.Text ?? "").Trim();
            if (tag.Length == 0) { Shell.Notify("Введите тег версии, например 1.9.9d.", LogLevel.Warn); return; }
            if (!System.Text.RegularExpressions.Regex.IsMatch(tag, "^v?[0-9][0-9a-zA-Z.]*$"))
            {
                Shell.Notify("Тег выглядит некорректно: " + tag, LogLevel.Warn);
                return;
            }
            ReleaseInfo rel = null;
            string error = null;
            Shell.RunBackground(delegate()
            {
                try { rel = GitHub.ReleaseByTag(GitHub.OfficialRepo, tag); }
                catch (Exception ex) { error = ex.Message; }
                Shell.Ui(delegate()
                {
                    if (error != null) { Shell.Notify("Не удалось получить версию " + tag + ": " + error, LogLevel.Error); return; }
                    InstallRelease(rel);
                });
            }, null);
        }

        private void InstallRelease(ReleaseInfo rel)
        {
            if (rel == null) return;
            string local = Shell.LocalVersion;
            string tag = Versions.NormalizeTag(rel.Tag);
            if (string.Equals(tag, local, StringComparison.OrdinalIgnoreCase))
            {
                Shell.Notify("Эта версия уже установлена.", LogLevel.Warn);
                return;
            }
            bool downgrade = Versions.Compare(tag, local) < 0;
            if (!Shell.RequireAdmin("Установка версии")) return;
            if (!Dlg.Confirm(this.View.FindForm(), downgrade ? "Откатиться на версию " + tag + "?" : "Установить версию " + tag + "?",
                string.Format("Будет скачана и установлена сборка {0} с официального репозитория Flowseal{1}.\n\nВаши списки, IP-лист и настройки сохраняются, bypass на время установки останавливается.",
                    rel.Tag, downgrade ? " (это более старая версия, чем установленная " + local + ")" : ""), "Установить")) return;

            _versionBusy = true;
            _btnVersionInstall.Enabled = false;
            _btnVersionLoad.Enabled = false;
            _versionProgress.Visible = true;
            _versionProgress.Value = 0;
            Shell.Form.SetStatusHint("Установка версии " + tag + "…");

            UpdateOptions opt = new UpdateOptions();
            opt.CreateBackup = _tgBackup.Checked;
            opt.RestartAfter = _tgRestart.Checked;
            opt.PreserveUserLists = true;
            opt.UpdateOfficialLists = _tgLists.Checked;

            Updater upd = new Updater(Shell.Install, Shell.Ops, delegate(UpdateProgress p)
            {
                Shell.Ui(delegate()
                {
                    _versionProgress.Value = p.Percent;
                    _versionHint.Text = p.Stage.Length > 0 ? p.Stage : _versionHint.Text;
                    if (!p.Finished) return;
                    _versionBusy = false;
                    _btnVersionInstall.Enabled = true;
                    _btnVersionLoad.Enabled = true;
                    _versionProgress.Visible = false;
                    Shell.Form.SetStatusHint("");
                    if (p.Ok)
                    {
                        _versionHint.Text = "Установлено: " + tag + ". " + p.Summary.Replace("\n", " ");
                        _versionHint.ForeColor = Theme.Success;
                        Shell.Notify("Версия " + tag + " установлена.", LogLevel.Ok);
                        Shell.Install.Rescan();
                        Shell.RefreshState(true);
                        UpdateChecker.Last = null;
                        _releases = null;
                    }
                    else
                    {
                        _versionHint.Text = "Ошибка: " + p.Error;
                        _versionHint.ForeColor = Theme.Warn;
                        Shell.Notify("Не удалось установить версию: " + p.Error, LogLevel.Error);
                    }
                });
            });
            Shell.RunBackground(delegate() { upd.UpdateNow(rel, opt); }, null);
        }
        private void BuildSelfCard()
        {
            _selfCard = NewCard("Обновление Zapret Manager", 188);
            _selfVersion = L("", Theme.Base, Theme.Text, 22, 42);
            _selfCard.Controls.Add(_selfVersion);
            _selfStatus = L("", Theme.Small, Theme.TextMuted, 22, 64);
            _selfCard.Controls.Add(_selfStatus);

            _btnSelfCheck = Btn("Проверить", ButtonKind.Secondary, 130);
            _btnSelfCheck.Location = new Point(22, 96);
            _btnSelfCheck.Click += delegate { CheckSelf(); };
            _selfCard.Controls.Add(_btnSelfCheck);

            _btnSelfUpdate = Btn("Обновить менеджер", ButtonKind.Primary, 190);
            _btnSelfUpdate.Location = new Point(162, 96);
            _btnSelfUpdate.Enabled = false;
            _btnSelfUpdate.Click += delegate { UpdateSelf(); };
            _selfCard.Controls.Add(_btnSelfUpdate);

            _selfProgress = new LineProgress();
            _selfProgress.Location = new Point(22, 142);
            _selfProgress.Width = DesignWidth - 44;
            _selfProgress.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right;
            _selfCard.Controls.Add(_selfProgress);

            _selfCard.Controls.Add(L("Источник: github.com/" + SelfUpdate.Repo + " • GitHub Releases, файл ZapretManager.exe с SHA-256",
                Theme.Small, Theme.TextMuted, 22, 156));
            View.Stack.Add(_selfCard);
        }

        private void CheckSelf()
        {
            _btnSelfCheck.Enabled = false;
            _selfStatus.Text = "Проверка обновлений…";
            Shell.RunBackground(delegate()
            {
                SelfUpdateInfo info = SelfUpdate.Check(true);
                Shell.Ui(delegate()
                {
                    _btnSelfCheck.Enabled = true;
                    ApplySelfInfo(info);
                    if (!info.Ok) Shell.Notify("Не удалось проверить обновление менеджера: " + info.Error, LogLevel.Warn);
                    else if (info.Available) Shell.Notify("Доступна новая версия менеджера " + info.Remote, LogLevel.Warn);
                    else Shell.Notify("Zapret Manager " + info.Local + " — актуальная версия.", LogLevel.Ok);
                });
            }, null);
        }

        private void ApplySelfInfo(SelfUpdateInfo info)
        {
            if (info == null)
            {
                _selfVersion.Text = "Zapret Manager " + AppInfo.Version;
                _selfStatus.Text = "Проверка не выполнялась.";
                _btnSelfUpdate.Enabled = false;
                return;
            }
            _selfVersion.Text = "Установлена версия " + info.Local;
            if (!info.Ok)
            {
                _selfStatus.Text = "Не удалось проверить: " + info.Error;
                _selfStatus.ForeColor = Theme.Warn;
                _btnSelfUpdate.Enabled = false;
                return;
            }
            if (info.Available)
            {
                _selfStatus.Text = "Доступна версия " + info.Remote + " — можно обновиться прямо отсюда.";
                _selfStatus.ForeColor = Theme.Warn;
                _btnSelfUpdate.Enabled = !_selfBusy;
            }
            else
            {
                _selfStatus.Text = "Установлена последняя версия.";
                _selfStatus.ForeColor = Theme.Success;
                _btnSelfUpdate.Enabled = false;
            }
        }

        private void UpdateSelf()
        {
            if (_selfBusy) return;
            SelfUpdateInfo info = SelfUpdate.Last;
            if (info == null || !info.Ok || !info.Available) { Shell.Notify("Сначала проверьте обновления.", LogLevel.Warn); return; }
            if (!Dlg.Confirm(this.View.FindForm(), "Обновить Zapret Manager?",
                string.Format("Будет скачана версия {0} из репозитория {1}.\n\nПриложение закроется, заменит файл и запустится снова. Настройки, списки и компоненты не затрагиваются.",
                    info.Remote, SelfUpdate.Repo), "Обновить")) return;

            _selfBusy = true;
            _btnSelfUpdate.Enabled = false;
            _btnSelfCheck.Enabled = false;
            _selfProgress.Value = 0;
            string remote = info.Remote;

            Shell.RunBackground(delegate()
            {
                bool ok;
                string pending;
                string msg = SelfUpdate.Apply(info, delegate(int percent, string stage)
                {
                    Shell.Ui(delegate() { _selfProgress.Value = percent; _selfStatus.Text = stage; });
                }, out ok, out pending);

                Shell.Ui(delegate()
                {
                    _selfBusy = false;
                    _btnSelfCheck.Enabled = true;
                    _selfProgress.Value = ok ? 100 : 0;
                    _selfStatus.Text = msg;
                    if (!ok)
                    {
                        _btnSelfUpdate.Enabled = true;
                        Shell.Notify(msg, LogLevel.Error);
                        return;
                    }
                    Shell.Notify("Обновление " + remote + " готово — перезапуск…", LogLevel.Ok);
                    Shell.RefreshState(false);
                    AppInfo.ExitRequested = true;
                    Application.Exit();
                });
            }, null);
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
            if (DlgHost.Show(dlg, this.View.FindForm()) == DialogResult.OK)
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
            if (DlgHost.Show(dlg, this.View.FindForm()) != DialogResult.OK) return;

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
            if (SelfUpdate.Last == null)
            {
                _selfVersion.Text = "Установлена версия " + AppInfo.Version;
                Shell.RunBackground(delegate()
                {
                    SelfUpdateInfo info = SelfUpdate.Check(false);
                    Shell.Ui(delegate() { ApplySelfInfo(info); });
                }, null);
            }
            else
            {
                ApplySelfInfo(SelfUpdate.Last);
            }
        }

        public override void RefreshState()
        {
            if (!_versionsTried && !_versionBusy && _releases == null)
            {
                _versionsTried = true;
                LoadVersions();
            }
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
