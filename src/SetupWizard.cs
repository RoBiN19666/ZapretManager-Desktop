using System;
using System.Drawing;
using System.IO;
using System.Windows.Forms;

namespace ZapretManager
{
    /// <summary>Мастер первой настройки: куда поставить zapret и TG WS Proxy и сразу их скачать.</summary>
    public sealed class SetupWizard : Form
    {
        private readonly Shell _shell;

        private Card _zapretCard;
        private Label _zapretStatus;
        private InputBox _zapretPath;
        private ToggleSwitch _tgZapret;
        private RoundButton _btnZapretBrowse;
        private RoundButton _btnZapretBat;

        private Card _tgCard;
        private Label _tgStatus;
        private InputBox _tgPath;
        private ToggleSwitch _tgTg;
        private RoundButton _btnTgBrowse;

        private LineProgress _progress;
        private Label _stage;
        private RoundButton _btnInstall;
        private RoundButton _btnSkip;
        private bool _busy;

        public SetupWizard(Shell shell)
        {
            _shell = shell;
            Text = "Первая настройка Zapret Manager";
            FormBorderStyle = FormBorderStyle.FixedDialog;
            StartPosition = FormStartPosition.CenterParent;
            MaximizeBox = false;
            MinimizeBox = false;
            ShowInTaskbar = false;
            BackColor = Theme.Bg;
            ForeColor = Theme.Text;
            Font = Theme.Base;
            AutoScaleMode = AutoScaleMode.None;
            ClientSize = new Size(760, 576);

            Controls.Add(L("Первая настройка", Theme.Title, Theme.Text, 28, 22));
            Controls.Add(WrapLabel("Отметьте, что нужно установить. Обе программы официальные и скачиваются с GitHub Flowseal. Позже это можно повторить из «Инструменты» → «Мастер установки».",
                Theme.Small, Theme.TextDim, 28, 56, 590));

            BuildZapretCard();
            BuildTgCard();

            _progress = new LineProgress();
            _progress.Location = new Point(28, 470);
            _progress.Width = 704;
            Controls.Add(_progress);

            _stage = L("", Theme.Small, Theme.TextMuted, 28, 484);
            Controls.Add(_stage);

            _btnSkip = Btn("Пропустить", ButtonKind.Secondary, 130);
            _btnSkip.Location = new Point(ClientSize.Width - 28 - 210 - 140, 522);
            _btnSkip.Click += delegate { DialogResult = DialogResult.Cancel; Close(); };
            Controls.Add(_btnSkip);

            _btnInstall = Btn("Установить и продолжить", ButtonKind.Primary, 210);
            _btnInstall.Location = new Point(ClientSize.Width - 28 - 210, 522);
            _btnInstall.Click += delegate { StartInstall(); };
            Controls.Add(_btnInstall);

            AcceptButton = _btnInstall;
            CancelButton = _btnSkip;

            Panel logo = new Panel();
            logo.Location = new Point(ClientSize.Width - 28 - 96, 2);
            logo.Size = new Size(96, 96);
            logo.BackColor = Color.Transparent;
            logo.Paint += delegate(object s, PaintEventArgs e)
            {
                e.Graphics.SmoothingMode = System.Drawing.Drawing2D.SmoothingMode.AntiAlias;
                Brand.DrawLock(e.Graphics, new Rectangle(0, 0, logo.Width, logo.Height), 255);
            };
            Controls.Add(logo);

            RefreshStatuses();
        }

        protected override void OnHandleCreated(EventArgs e)
        {
            base.OnHandleCreated(e);
            Native.EnableDarkTitleBar(Handle);
        }

        private static Label L(string text, Font font, Color color, int x, int y)
        {
            Label l = new Label();
            l.Text = text;
            l.Font = font;
            l.ForeColor = color;
            l.BackColor = Color.Transparent;
            l.AutoSize = true;
            l.Location = new Point(x, y);
            return l;
        }

        private static Label WrapLabel(string text, Font font, Color color, int x, int y, int width)
        {
            Label l = new Label();
            l.Text = text;
            l.Font = font;
            l.ForeColor = color;
            l.BackColor = Color.Transparent;
            l.AutoSize = false;
            l.Width = width;
            l.Height = TextRenderer.MeasureText(text, font, new Size(width, 400), TextFormatFlags.WordBreak).Height + 4;
            l.Location = new Point(x, y);
            return l;
        }

        private static RoundButton Btn(string text, ButtonKind kind, int width)
        {
            RoundButton b = new RoundButton();
            b.Text = text;
            b.Kind = kind;
            b.Width = width;
            b.Height = 34;
            return b;
        }

        private void BuildZapretCard()
        {
            _zapretCard = new Card("Запрет (обход DPI для Discord и YouTube)", 168);
            _zapretCard.Width = 704;
            _zapretCard.Location = new Point(28, 112);
            Controls.Add(_zapretCard);

            _zapretStatus = L("", Theme.Small, Theme.TextDim, 22, 40);
            _zapretStatus.AutoSize = false;
            _zapretStatus.AutoEllipsis = true;
            _zapretCard.Controls.Add(_zapretStatus);
            _zapretStatus.Bounds = new Rectangle(22, 40, 660, 16);

            _zapretCard.Controls.Add(L("Папка для установки:", Theme.Small, Theme.TextMuted, 22, 62));
            _zapretPath = new InputBox();
            _zapretPath.Location = new Point(22, 80);
            _zapretPath.Width = 400;
            _zapretPath.Text = DefaultZapretParent();
            _zapretCard.Controls.Add(_zapretPath);

            _btnZapretBrowse = Btn("Обзор…", ButtonKind.Secondary, 96);
            _btnZapretBrowse.Location = new Point(428, 80);
            _btnZapretBrowse.Click += delegate { BrowseFolder(_zapretPath); };
            _zapretCard.Controls.Add(_btnZapretBrowse);

            _btnZapretBat = Btn("Указать service.bat…", ButtonKind.Ghost, 176);
            _btnZapretBat.Location = new Point(530, 80);
            _btnZapretBat.Click += delegate { PickServiceBat(); };
            _zapretCard.Controls.Add(_btnZapretBat);

            Label zapretToggleLabel = L("Скачать и установить (последняя версия)", Theme.Small, Theme.TextDim, 22, 126);
            _zapretCard.Controls.Add(zapretToggleLabel);
            _tgZapret = new ToggleSwitch();
            _tgZapret.Checked = true;
            _tgZapret.Location = new Point(zapretToggleLabel.Right + 14, 121);
            _zapretCard.Controls.Add(_tgZapret);
        }

        private void BuildTgCard()
        {
            _tgCard = new Card("TG WS Proxy (ускорение Telegram через WebSocket)", 168);
            _tgCard.Width = 704;
            _tgCard.Location = new Point(28, 292);
            Controls.Add(_tgCard);

            _tgStatus = L("", Theme.Small, Theme.TextDim, 22, 40);
            _tgStatus.AutoSize = false;
            _tgStatus.AutoEllipsis = true;
            _tgStatus.Bounds = new Rectangle(22, 40, 660, 16);
            _tgCard.Controls.Add(_tgStatus);

            _tgCard.Controls.Add(L("Папка для установки:", Theme.Small, Theme.TextMuted, 22, 62));
            _tgPath = new InputBox();
            _tgPath.Location = new Point(22, 80);
            _tgPath.Width = 400;
            _tgPath.Text = TgProxy.DefaultDir;
            _tgCard.Controls.Add(_tgPath);

            _btnTgBrowse = Btn("Обзор…", ButtonKind.Secondary, 96);
            _btnTgBrowse.Location = new Point(428, 80);
            _btnTgBrowse.Click += delegate { BrowseFolder(_tgPath); };
            _tgCard.Controls.Add(_btnTgBrowse);

            Label tgToggleLabel = L("Скачать и установить (~21 МБ)", Theme.Small, Theme.TextDim, 22, 126);
            _tgCard.Controls.Add(tgToggleLabel);
            _tgTg = new ToggleSwitch();
            _tgTg.Checked = true;
            _tgTg.Location = new Point(tgToggleLabel.Right + 14, 121);
            _tgCard.Controls.Add(_tgTg);
        }

        private string DefaultZapretParent()
        {
            string local = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "ZapretManager");
            return Path.Combine(local, "zapret");
        }

        private void BrowseFolder(InputBox box)
        {
            FolderBrowserDialog dlg = new FolderBrowserDialog();
            dlg.Description = "Выберите папку";
            try
            {
                string current = box.Text.Trim();
                if (current.Length > 0) dlg.SelectedPath = Directory.Exists(current) ? current : Path.GetDirectoryName(current);
            }
            catch { }
            if (dlg.ShowDialog(this) == DialogResult.OK) box.Text = dlg.SelectedPath;
        }

        private void PickServiceBat()
        {
            OpenFileDialog dlg = new OpenFileDialog();
            dlg.Title = "Выберите service.bat в папке уже установленного zapret";
            dlg.Filter = "service.bat|service.bat|Все файлы (*.*)|*.*";
            if (dlg.ShowDialog(this) != DialogResult.OK) return;
            string root = Path.GetDirectoryName(dlg.FileName);
            if (ZapretInstall.LooksValid(root))
            {
                _shell.SetRoot(root);
                RefreshStatuses();
                _stage.Text = "Используется установка: " + root;
            }
            else
            {
                MessageBox.Show(this, "В выбранной папке нет service.bat и bin\\winws.exe.", Text, MessageBoxButtons.OK, MessageBoxIcon.Warning);
            }
        }

        private void RefreshStatuses()
        {
            bool zapretOk = _shell.Install != null && _shell.Install.IsValid;
            if (zapretOk)
            {
                _zapretStatus.Text = string.Format("Уже установлен: {0} (версия {1})", _shell.Install.Root, _shell.Install.Version);
                _zapretStatus.ForeColor = Theme.Success;
                _tgZapret.Checked = false;
                _tgZapret.Enabled = false;
                _zapretPath.Text = _shell.Install.Root;
                _zapretPath.Box.Enabled = false;
                _btnZapretBrowse.Enabled = false;
            }
            else
            {
                _zapretStatus.Text = "Не найден — можно скачать и установить сейчас.";
                _zapretStatus.ForeColor = Theme.Warn;
                _tgZapret.Enabled = true;
                _zapretPath.Box.Enabled = true;
                _btnZapretBrowse.Enabled = true;
            }

            TgProxyState tg = TgProxy.GetState(_shell.Config);
            bool zapretReady = zapretOk;
            if (tg.Installed)
            {
                _tgStatus.Text = string.Format("Уже установлен: {0}{1}", tg.Dir, tg.Version.Length > 0 ? " (версия " + tg.Version + ")" : "");
                _tgStatus.ForeColor = Theme.Success;
                _tgTg.Checked = false;
                _tgTg.Enabled = false;
            }
            else
            {
                _tgStatus.Text = string.Format("Не найден. Прокси слушает {0}:1443 и сворачивается в трей — им можно управлять отсюда.", "127.0.0.1");
                _tgStatus.ForeColor = Theme.Warn;
                _tgTg.Enabled = true;
            }
            _btnInstall.Text = (zapretOk && tg.Installed) ? "Готово" : "Установить и продолжить";
            _btnInstall.Fit(10);
        }

        private void StartInstall()
        {
            if (_busy) return;
            if (!_tgZapret.Checked && _shell.Install != null && _shell.Install.IsValid && !_tgTg.Checked)
            {
                DialogResult = DialogResult.OK;
                Close();
                return;
            }
            if (!Native.IsAdmin())
            {
                MessageBox.Show(this, "Для установки нужны права администратора.", Text, MessageBoxButtons.OK, MessageBoxIcon.Warning);
                return;
            }
            _busy = true;
            _btnInstall.Enabled = false;
            _btnSkip.Enabled = false;
            _tgZapret.Enabled = false;
            _tgTg.Enabled = false;
            _progress.Value = 0;

            bool installZapret = _tgZapret.Checked;
            bool installTg = _tgTg.Checked;
            string zapretParent = _zapretPath.Text.Trim();
            string tgDir = _tgPath.Text.Trim();

            System.Threading.Thread t = new System.Threading.Thread(delegate()
            {
                try
                {
                    if (installZapret)
                    {
                        SetStage("Получение информации о последней версии zapret…", 3);
                        ReleaseInfo rel = null;
                        try { rel = GitHub.GetLatestRelease(GitHub.OfficialRepo); }
                        catch (Exception) { rel = GitHub.GetLatestVersionFallback(); }
                        if (!Directory.Exists(zapretParent)) Directory.CreateDirectory(zapretParent);
                        SetStage("Скачивание zapret " + rel.Tag + "…", 5);
                        ZapretInstall inst = Updater.InstallFresh(rel, zapretParent, delegate(int percent, string stage)
                        {
                            SetStage(stage, installTg ? 5 + percent / 2 : 5 + percent);
                        });
                        Ui(delegate()
                        {
                            _shell.SetRoot(inst.Root);
                            RefreshStatuses();
                        });
                    }
                    if (installTg)
                    {
                        SetStage("Проверка версии TG WS Proxy…", installZapret ? 55 : 5);
                        ReleaseInfo rel = TgProxy.CheckLatest();
                        string msg;
                        bool ok;
                        msg = TgProxy.Install(_shell.Config, rel.Tag, tgDir, delegate(int percent, string stage)
                        {
                            int basePercent = installZapret ? 55 : 5;
                            SetStage(stage, basePercent + (int)(percent * (100 - basePercent) / 100.0));
                        }, out ok);
                        Log.Info("Мастер установки: " + msg);
                        Ui(delegate() { RefreshStatuses(); });
                    }
                    Ui(delegate()
                    {
                        _busy = false;
                        _btnInstall.Enabled = true;
                        _btnSkip.Enabled = true;
                        _stage.Text = "Готово.";
                        _progress.Value = 100;
                        DialogResult = DialogResult.OK;
                        Close();
                    });
                }
                catch (Exception ex)
                {
                    Log.Error("Мастер установки: ошибка", ex);
                    Ui(delegate()
                    {
                        _busy = false;
                        _btnInstall.Enabled = true;
                        _btnSkip.Enabled = true;
                        _stage.Text = "Ошибка: " + ex.Message;
                        _progress.Value = 0;
                        MessageBox.Show(this, "Установка не завершена:\n\n" + ex.Message, Text, MessageBoxButtons.OK, MessageBoxIcon.Error);
                    });
                }
            });
            t.IsBackground = true;
            t.Start();
        }

        private void SetStage(string text, int percent)
        {
            if (_stage == null) return;
            Ui(delegate()
            {
                _stage.Text = text;
                _progress.Value = Math.Max(0, Math.Min(100, percent));
            });
        }

        private void Ui(Action action)
        {
            if (IsDisposed || !IsHandleCreated) return;
            if (InvokeRequired) BeginInvoke((MethodInvoker)delegate() { action(); });
            else action();
        }
    }
}
