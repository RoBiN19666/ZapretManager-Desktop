using System;
using System.Collections.Generic;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Threading;
using System.Windows.Forms;

namespace ZapretManager
{
    /// <summary>Общий контекст приложения, доступный всем страницам.</summary>
    public sealed class Shell
    {
        public MainForm Form;
        public AppConfig Config;
        public ZapretInstall Install;
        public ZapretOps Ops;
        public RuntimeState State;
        public UpdateCheckResult UpdateState;
        public List<PageBase> Pages = new List<PageBase>();
        public PageBase Current;
        public bool Busy;
        public int BackgroundJobs;

        public event Action StateChanged;
        public event Action UpdateStateChanged;

        public string LocalVersion
        {
            get { return Install != null && Install.IsValid ? (Install.Version ?? "") : ""; }
        }

        public void Notify(string message, LogLevel level)
        {
            Log.Write(level, message);
            if (Form != null) Form.ShowToast(message, level);
        }

        public bool RequireAdmin(string action)
        {
            if (Native.IsAdmin()) return true;
            Notify(action + ": нужны права администратора. Перезапустите программу от имени администратора.", LogLevel.Warn);
            return false;
        }

        public void RefreshState(bool rescanFiles)
        {
            if (Install == null || Ops == null) return;
            try
            {
                if (rescanFiles && Install.IsValid) Install.Rescan();
                State = Ops.GetState();
            }
            catch (Exception ex)
            {
                Log.Error("Ошибка получения состояния", ex);
                return;
            }
            if (Form != null) Form.UpdateStatus();
            if (Current != null) Current.RefreshState();
            Action h = StateChanged;
            if (h != null) h();
        }

        /// <summary>Обновление состояния в фоне (чтобы не подтормаживал интерфейс).</summary>
        private int _refreshing;

        public void RefreshStateAsync()
        {
            if (Interlocked.CompareExchange(ref _refreshing, 1, 0) != 0) return;
            ZapretOps ops = Ops;
            if (ops == null) { Interlocked.Exchange(ref _refreshing, 0); return; }
            ThreadPool.QueueUserWorkItem(delegate(object state)
            {
                RuntimeState st = null;
                try { st = ops.GetState(); }
                catch (Exception ex) { Log.Error("Ошибка получения состояния", ex); }
                Ui(delegate()
                {
                    try
                    {
                        if (st != null)
                        {
                            State = st;
                            if (Form != null) Form.UpdateStatus();
                            if (Current != null) Current.RefreshState();
                            Action h = StateChanged;
                            if (h != null) h();
                        }
                    }
                    finally { Interlocked.Exchange(ref _refreshing, 0); }
                });
            });
        }

        public void RaiseUpdateStateChanged()
        {
            Action h = UpdateStateChanged;
            if (h != null) h();
        }

        /// <summary>Меняет папку установки zapret.</summary>
        public void SetRoot(string root)
        {
            ZapretInstall inst = new ZapretInstall();
            inst.Root = root != null ? root.TrimEnd('\\') : "";
            inst.Rescan();
            Install = inst;
            Ops = new ZapretOps(inst);
            Config.Set("Root", inst.Root);
            Config.Save();
            if (Form != null) Form.RebuildPages();
            Log.Info("Папка zapret: " + inst.Root + (inst.IsValid ? "" : " (не найдена)"));
            RefreshState(false);
        }

        /// <summary>Фоновая операция с индикацией занятости.</summary>
        public void RunBackground(ThreadStart work, Action onDone)
        {
            Busy = true;
            BackgroundJobs++;
            if (Form != null) Form.UpdateBusy();
            Thread t = new Thread(delegate()
            {
                try { work(); }
                catch (Exception ex) { Log.Error("Ошибка фоновой операции", ex); }
                finally
                {
                    if (Form != null)
                    {
                        try
                        {
                            Form.BeginInvoke((MethodInvoker)delegate()
                            {
                                BackgroundJobs--;
                                Busy = BackgroundJobs > 0;
                                Form.UpdateBusy();
                                if (onDone != null) onDone();
                            });
                        }
                        catch { }
                    }
                }
            });
            t.IsBackground = true;
            t.Name = "zapret-bg";
            t.Start();
        }

        /// <summary>Выполнить действие в UI-потоке.</summary>
        public void Ui(Action action)
        {
            if (Form == null) { action(); return; }
            if (Form.InvokeRequired) Form.BeginInvoke((MethodInvoker)delegate() { action(); });
            else action();
        }
    }

    public abstract class PageBase
    {
        protected Shell Shell;
        public ScrollHost View = new ScrollHost();

        protected PageBase(Shell shell)
        {
            Shell = shell;
            View.Dock = DockStyle.Fill;
        }

        public abstract string Key { get; }
        public abstract string Title { get; }
        public abstract string Subtitle { get; }
        public abstract string Icon { get; }

        public abstract void Build();
        public virtual void OnShow() { }
        public virtual void RefreshState() { }

        public const int DesignWidth = 880;

        protected Card NewCard(string header, int height)
        {
            Card c = new Card(header, height);
            c.Width = DesignWidth;
            return c;
        }

        protected static Label L(string text, Font font, Color color, int x, int y)
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

        protected static Label WrapLabel(string text, Font font, Color color, int x, int y, int width)
        {
            Label l = new Label();
            l.Text = text;
            l.Font = font;
            l.ForeColor = color;
            l.BackColor = Color.Transparent;
            l.AutoSize = false;
            l.Width = width;
            l.Height = TextRenderer.MeasureText(text, font, new Size(width, 2000), TextFormatFlags.WordBreak).Height + 4;
            l.Location = new Point(x, y);
            return l;
        }

        protected static RoundButton Btn(string text, ButtonKind kind, int width)
        {
            RoundButton b = new RoundButton();
            b.Text = text;
            b.Kind = kind;
            if (width <= 0) b.Fit(6);
            else b.Width = width;
            b.Height = 32;
            return b;
        }

        protected static void Add(Card card, params Control[] children)
        {
            foreach (Control c in children) card.Controls.Add(c);
        }
    }

    public sealed class MainForm : Form
    {
        private readonly Shell _shell;
        private Panel _sidebar;
        private Panel _content;
        private Panel _pageHost;
        private bool _chromeScaled;
        private Panel _statusBar;
        private Label _titleLbl;
        private Label _subtitleLbl;
        private Label _statusLeft;
        private Label _statusRight;
        private List<NavButton> _navButtons = new List<NavButton>();
        private Panel _toast;
        private Label _toastLabel;
        private System.Windows.Forms.Timer _toastTimer;
        private System.Windows.Forms.Timer _tickTimer;
        private StatusDot _dotBypass;
        private StatusDot _dotService;
        private Label _lblBypass;
        private Label _lblService;
        private Label _lblIpSet;
        private Pill _adminPill;
        private Panel _logoBox;
        private Label _navGroup1;
        private Label _navGroup2;

        public const int SidebarWidth = 236;

        public MainForm(Shell shell)
        {
            _shell = shell;
            shell.Form = this;
            Text = "Zapret Manager";
            BackColor = Theme.Bg;
            Font = Theme.Base;
            ClientSize = new Size(1280, 880);
            MinimumSize = new Size(1180, 740);
            StartPosition = FormStartPosition.CenterScreen;
            DoubleBuffered = true;
            KeyPreview = true;
            AutoScaleMode = AutoScaleMode.None;
            try { Icon = AppIcon.Get(); } catch { }

            BuildChrome();
            BuildSidebarBase();
            BuildContent();
            BuildStatusBar();
            BuildToast();
            LayoutChrome();

            _tickTimer = new System.Windows.Forms.Timer();
            _tickTimer.Interval = 5000;
            _tickTimer.Tick += delegate
            {
                if (_shell.Busy) return;
                _shell.RefreshStateAsync();
            };
            _tickTimer.Start();
        }

        protected override void OnShown(EventArgs e)
        {
            base.OnShown(e);
            Native.SetTitleBarTheme(Handle, Theme.IsDark);
            BuildTray();
            StartActivationWatcher();

            // реальный DPI берём у окна: приложение могло открыться на мониторе с другим масштабом
            float windowDpi = 96f;
            try { using (Graphics g = CreateGraphics()) windowDpi = g.DpiX; } catch { }
            float uiScale = 1f;
            float.TryParse(_shell.Config.Get("UiScale", "100"), out uiScale);
            Screen screen = Screen.FromControl(this);
            Log.Info(string.Format("Монитор: {0}, область {1}x{2}, DPI окна {3:0}", screen.DeviceName, screen.WorkingArea.Width, screen.WorkingArea.Height, windowDpi));
            Theme.InitFonts(uiScale / 100f, windowDpi);
            ApplyLayoutScale();
            RebuildPages();
            ScaleChrome();
            LayoutChrome();
            _shell.RefreshState(true);
            StartUpdateCheck();
            if (!string.IsNullOrEmpty(AppInfo.ShotPath)) { CaptureAndExit(); return; }
            if (AppInfo.AutostartMode)
            {
                RunAutostartBootstrap();
                HideToTray(true);
                return;
            }
            if (AppInfo.TrayMode)
            {
                HideToTray(false);
                return;
            }
            if (_shell.Config.Get("SetupDone", "").Length == 0) RunSetupWizard();
        }

        private System.Threading.Thread _activationThread;

        /// <summary>Слушает запросы второго экземпляра «покажи окно».</summary>
        private void StartActivationWatcher()
        {
            if (AppInfo.ActivateEvent == null) return;
            _activationThread = new System.Threading.Thread(delegate()
            {
                while (true)
                {
                    bool signaled = false;
                    try { signaled = AppInfo.ActivateEvent.WaitOne(400); }
                    catch { return; }
                    if (!signaled) continue;
                    try
                    {
                        BeginInvoke((MethodInvoker)delegate()
                        {
                            ShowFromTray();
                            WindowState = FormWindowState.Normal;
                        });
                    }
                    catch { return; }
                }
            });
            _activationThread.IsBackground = true;
            _activationThread.Name = "zapret-activate";
            _activationThread.Start();
        }
        // ---------- трей и фоновый режим ----------

        private NotifyIcon _tray;
        private bool _trayHintShown;

        private void BuildTray()
        {
            if (_tray != null) return;
            _tray = new NotifyIcon();
            try { _tray.Icon = Icon; } catch { _tray.Icon = SystemIcons.Application; }
            _tray.Text = "Zapret Manager";
            _tray.Visible = true;
            _tray.DoubleClick += delegate { ShowFromTray(); };

            DarkMenu menu = new DarkMenu();
            menu.BackColor = Theme.Surface;
            menu.ForeColor = Theme.Text;
            menu.Renderer = new DarkMenuRenderer();
            menu.Items.Add("Открыть Zapret Manager", null, delegate { ShowFromTray(); });
            ToolStripMenuItem zapretItem = new ToolStripMenuItem("Запрет (обход DPI)");
            zapretItem.Click += delegate { TrayToggleZapret(); };
            ToolStripMenuItem tgItem = new ToolStripMenuItem("TG WS Proxy");
            tgItem.Click += delegate { TrayToggleTgProxy(); };
            menu.Items.Add(zapretItem);
            menu.Items.Add(tgItem);
            menu.Items.Add(new ToolStripSeparator());
            menu.Items.Add("Проверить обновления", null, delegate
            {
                ShowFromTray();
                Navigate("update");
            });
            menu.Items.Add(new ToolStripSeparator());
            menu.Items.Add("Выход", null, delegate { ExitApplication(); });
            // иконки и отступы: картинку ставит сам пункт, поэтому выравнивание штатное
            foreach (ToolStripItem item in menu.Items)
            {
                if (item is ToolStripSeparator) continue;
                item.Font = Theme.Base;
                item.Padding = new Padding(Theme.Px(2), Theme.Px(7), Theme.Px(18), Theme.Px(7));
                item.ImageScaling = ToolStripItemImageScaling.None;
                if (item.Text.StartsWith("Открыть")) item.Image = MenuIcons.Get("open");
                else if (item.Text.StartsWith("Запрет")) item.Image = MenuIcons.Get("shield");
                else if (item.Text.StartsWith("TG")) item.Image = MenuIcons.Get("telegram");
                else if (item.Text.StartsWith("Проверить")) item.Image = MenuIcons.Get("refresh");
                else if (item.Text.StartsWith("Выход")) item.Image = MenuIcons.Get("exit");
            }
            menu.Opening += delegate
            {
                bool zapretOn = _shell.State != null && _shell.State.WinwsRunning;
                zapretItem.Text = zapretOn ? "Остановить zapret" : "Запустить zapret";
                bool tgOn = false;
                try { tgOn = TgProxy.GetState(_shell.Config).Running; } catch { }
                tgItem.Text = tgOn ? "Остановить TG WS Proxy" : "Запустить TG WS Proxy";
            };
            // позиционируем сами, но координатами WinForms: они уже пересчитаны
            // в систему координат процесса, в отличие от «сырого» GetCursorPos
            _tray.MouseUp += delegate(object s, MouseEventArgs e)
            {
                if (e.Button != MouseButtons.Right) return;

                // состояние окна до показа меню: меню активирует процесс, и Windows
                // восстанавливает свёрнутое (или скрытое) окно — возвращаем как было
                bool wasHidden = !Visible;
                bool wasMinimized = WindowState == FormWindowState.Minimized;

                System.Drawing.Point at = Native.CursorPosition();
                menu.Show(at);
                // фокус меню обязателен: без него оно не закрывается по клику вне его
                Native.SetForegroundWindow(menu.Handle);

                BeginInvoke((MethodInvoker)delegate
                {
                    if (wasHidden && Visible)
                    {
                        Hide();
                        ShowInTaskbar = false;
                    }
                    else if (wasMinimized && WindowState != FormWindowState.Minimized)
                    {
                        WindowState = FormWindowState.Minimized;
                    }
                });

                Log.Info(string.Format("Меню трея: курсор {0},{1}, меню {2},{3} {4}x{5}, скрыто: {6}, свёрнуто: {7}",
                    at.X, at.Y, menu.Bounds.Left, menu.Bounds.Top, menu.Bounds.Width, menu.Bounds.Height, wasHidden, wasMinimized));
            };
        }

        private void TrayToggleZapret()
        {
            if (_shell.State != null && _shell.State.WinwsRunning)
            {
                _shell.Ops.Stop();
                _shell.Notify("Bypass остановлен.", LogLevel.Info);
            }
            else
            {
                StrategyDef s = StrategyPicker.Pick(_shell);
                if (s == null) { _shell.Notify("Стратегии не найдены.", LogLevel.Error); return; }
                ZapretOps.StartResult r = _shell.Ops.Start(s);
                _shell.Notify(r.Message, r.Ok ? LogLevel.Ok : LogLevel.Error);
            }
            _shell.RefreshStateAsync();
        }

        private void TrayToggleTgProxy()
        {
            TgProxyState st = TgProxy.GetState(_shell.Config);
            if (st.Running)
            {
                _shell.Notify(TgProxy.Stop(), LogLevel.Info);
            }
            else
            {
                bool ok;
                string msg = TgProxy.Start(_shell.Config, out ok);
                _shell.Notify(msg, ok ? LogLevel.Ok : LogLevel.Error);
            }
            _shell.RefreshStateAsync();
        }

        public void HideToTray(bool silent)
        {
            Hide();
            ShowInTaskbar = false;
            if (!silent && !_trayHintShown && _tray != null)
            {
                _trayHintShown = true;
                try { _tray.ShowBalloonTip(2500, "Zapret Manager", "Свёрнуто в трей — приложение продолжает работать.", ToolTipIcon.Info); } catch { }
            }
        }

        public void ShowFromTray()
        {
            ShowInTaskbar = true;
            Show();
            WindowState = FormWindowState.Normal;
            Activate();
            Native.SetForegroundWindow(Handle);
        }

        public void ExitApplication()
        {
            AppInfo.ExitRequested = true;
            Close();
        }

        /// <summary>Автозапуск: поднять подключённые сервисы и остаться в трее.</summary>
        private void RunAutostartBootstrap()
        {
            _shell.RunBackground(delegate()
            {
                bool startZapret = _shell.Config.GetBool("AutostartZapret", true);
                bool startTg = _shell.Config.GetBool("AutostartTgProxy", true);
                if (startZapret && _shell.Install != null && _shell.Install.IsValid)
                {
                    RuntimeState state = _shell.Ops.GetState();
                    ServiceSnapshot svc = ZapretOps.QueryService("zapret");
                    bool serviceRuns = svc.Installed && svc.Status == "работает";
                    if (!state.WinwsRunning && !serviceRuns)
                    {
                        StrategyDef s = StrategyPicker.Pick(_shell);
                        if (s != null) _shell.Ops.Start(s);
                    }
                    else if (serviceRuns) Log.Info("Автозапуск: служба zapret уже работает.");
                }
                if (startTg)
                {
                    TgProxyState tg = TgProxy.GetState(_shell.Config);
                    if (tg.Installed && !tg.Running)
                    {
                        bool ok;
                        TgProxy.Start(_shell.Config, out ok);
                    }
                }
                _shell.Ui(delegate() { _shell.RefreshState(false); });
            }, null);
        }

        private void RunSetupWizard()
        {
            using (SetupWizard wizard = new SetupWizard(_shell))
            {
                wizard.ShowDialog(this);
            }
            _shell.Config.Set("SetupDone", DateTime.Now.ToString("yyyy-MM-dd HH:mm"));
            _shell.Config.Save();
            if (_shell.Install != null && _shell.Install.IsValid) RebuildPages();
            _shell.RefreshState(true);
        }

        /// <summary>Служебный режим: сохранить снимок окна в PNG и закрыться (--shot=file).</summary>
        private void CaptureAndExit()
        {
            if (!string.IsNullOrEmpty(AppInfo.ShotPage))
            {
                if (AppInfo.ShotPage == "dialog")
                {
                    CaptureDialog();
                    return;
                }
                if (AppInfo.ShotPage == "wizard")
                {
                    CaptureWizard();
                    return;
                }
                Navigate(AppInfo.ShotPage);
                if (AppInfo.ShotScroll > 0 && _shell.Current != null)
                {
                    // даём странице разложиться: иначе максимум прокрутки ещё нулевой
                    _shell.Current.View.Stack.PerformLayout();
                    _shell.Current.View.PerformLayout();
                    Application.DoEvents();
                    _shell.Current.View.Stack.PerformLayout();
                    _shell.Current.View.ScrollTo(AppInfo.ShotScroll);
                    Application.DoEvents();
                }
            }
            TopMost = true;
            StartPosition = FormStartPosition.Manual;
            WindowState = FormWindowState.Normal;
            Native.ShowWindow(Handle, 9);
            Rectangle wa = Screen.PrimaryScreen.WorkingArea;
            Size = new Size(Math.Min(Theme.Px(1280), wa.Width - 20), Math.Min(Theme.Px(880), wa.Height - 20));
            Location = new Point(wa.Left + 10, wa.Top + 10);
            BringToFront();
            Activate();
            Native.SetForegroundWindow(Handle);
            System.Windows.Forms.Timer t = new System.Windows.Forms.Timer();
            t.Interval = 1200;
            t.Tick += delegate
            {
                t.Stop();
                try
                {
                    for (int attempt = 0; attempt < 8; attempt++)
                    {
                        Native.ShowWindow(Handle, 9);
                        WindowState = FormWindowState.Normal;
                        Location = new Point(Screen.PrimaryScreen.WorkingArea.Left + 10, Screen.PrimaryScreen.WorkingArea.Top + 10);
                        BringToFront();
                        Native.SetForegroundWindow(Handle);
                        Application.DoEvents();
                        if (Native.GetForegroundWindow() == Handle) break;
                        System.Threading.Thread.Sleep(250);
                    }
                    Log.Info(string.Format("shot: visible={0} state={1} bounds={2} client={3} fg={4} screen={5}",
                        Visible, WindowState, Bounds, ClientSize, Native.GetForegroundWindow(), PointToScreen(new Point(0, 0))));
                    _shell.RefreshState(false);
                    PerformLayout();
                    Application.DoEvents();
                    System.Threading.Thread.Sleep(600);
                    Application.DoEvents();
                    Bitmap bmp = new Bitmap(ClientSize.Width, ClientSize.Height);
                    bool printed = false;
                    using (Graphics g = Graphics.FromImage(bmp))
                    {
                        IntPtr hdc = g.GetHdc();
                        try { printed = Native.PrintWindow(Handle, hdc, 1); }
                        finally { g.ReleaseHdc(hdc); }
                        if (!printed) g.CopyFromScreen(PointToScreen(new Point(0, 0)), new Point(0, 0), ClientSize);
                    }
                    Log.Info("shot: PrintWindow=" + printed);
                    bmp.Save(AppInfo.ShotPath, System.Drawing.Imaging.ImageFormat.Png);
                    bmp.Dispose();
                    Log.Info("Снимок окна сохранён: " + AppInfo.ShotPath);
                }
                catch (Exception ex) { Log.Error("Не удалось сохранить снимок окна", ex); }
                AppInfo.ExitRequested = true;
                Close();
            };
            t.Start();
        }

        /// <summary>Служебный режим: снимок мастера установки.</summary>
        private void CaptureWizard()
        {
            SetupWizard wiz = new SetupWizard(_shell);
            wiz.StartPosition = FormStartPosition.Manual;
            wiz.Location = new Point(Screen.PrimaryScreen.WorkingArea.Left + 80, Screen.PrimaryScreen.WorkingArea.Top + 60);
            wiz.Show(this);
            wiz.BringToFront();
            System.Windows.Forms.Timer t = new System.Windows.Forms.Timer();
            t.Interval = 1100;
            t.Tick += delegate
            {
                t.Stop();
                try
                {
                    Application.DoEvents();
                    int frameW = Math.Max(0, wiz.Width - wiz.ClientSize.Width);
                    int frameH = Math.Max(0, wiz.Height - wiz.ClientSize.Height);
                    using (Bitmap full = new Bitmap(wiz.Width, wiz.Height))
                    {
                        wiz.DrawToBitmap(full, new Rectangle(0, 0, wiz.Width, wiz.Height));
                        using (Bitmap bmp = new Bitmap(wiz.ClientSize.Width, wiz.ClientSize.Height))
                        {
                            using (Graphics g = Graphics.FromImage(bmp))
                            {
                                g.DrawImage(full, new Rectangle(0, 0, bmp.Width, bmp.Height),
                                    new Rectangle(frameW / 2, frameH, bmp.Width, bmp.Height), GraphicsUnit.Pixel);
                            }
                            bmp.Save(AppInfo.ShotPath, System.Drawing.Imaging.ImageFormat.Png);
                        }
                    }
                    Log.Info("Снимок мастера сохранён: " + AppInfo.ShotPath);
                }
                catch (Exception ex) { Log.Error("Не удалось сохранить снимок мастера", ex); }
                wiz.Close();
                AppInfo.ExitRequested = true;
                Close();
            };
            t.Start();
        }
        /// <summary>Служебный режим: снимок диалога подтверждения.</summary>
        private void CaptureDialog()
        {
            DialogForm dlg = new DialogForm("Установить службу?",
                "Стратегия «general (ALT10)» будет установлена как служба Windows «zapret» и будет запускаться автоматически при загрузке. Текущий запущенный bypass будет остановлен.",
                "Установить", false, true);
            dlg.StartPosition = FormStartPosition.Manual;
            dlg.Location = new Point(Screen.PrimaryScreen.WorkingArea.Left + 120, Screen.PrimaryScreen.WorkingArea.Top + 120);
            dlg.Show(this);
            dlg.BringToFront();
            System.Windows.Forms.Timer t = new System.Windows.Forms.Timer();
            t.Interval = 900;
            t.Tick += delegate
            {
                t.Stop();
                try
                {
                    Application.DoEvents();
                    // DrawToBitmap рисует окно вместе с рамкой — вырезаем клиентскую часть
                    int frameW = Math.Max(0, dlg.Width - dlg.ClientSize.Width);
                    int frameH = Math.Max(0, dlg.Height - dlg.ClientSize.Height);
                    using (Bitmap full = new Bitmap(dlg.Width, dlg.Height))
                    {
                        dlg.DrawToBitmap(full, new Rectangle(0, 0, dlg.Width, dlg.Height));
                        using (Bitmap bmp = new Bitmap(dlg.ClientSize.Width, dlg.ClientSize.Height))
                        {
                            using (Graphics g = Graphics.FromImage(bmp))
                            {
                                g.DrawImage(full, new Rectangle(0, 0, bmp.Width, bmp.Height),
                                    new Rectangle(frameW / 2, frameH, bmp.Width, bmp.Height), GraphicsUnit.Pixel);
                            }
                            bmp.Save(AppInfo.ShotPath, System.Drawing.Imaging.ImageFormat.Png);
                        }
                        Log.Info("Снимок диалога сохранён: " + AppInfo.ShotPath);
                    }
                }
                catch (Exception ex) { Log.Error("Не удалось сохранить снимок диалога", ex); }
                dlg.Close();
                AppInfo.ExitRequested = true;
                Close();
            };
            t.Start();
        }

        protected override void OnFormClosing(FormClosingEventArgs e)
        {
            bool explicitExit = AppInfo.ExitRequested || string.IsNullOrEmpty(AppInfo.ShotPath) == false;
            bool trayEnabled = _shell.Config.GetBool("CloseToTray", true);
            if (!explicitExit && trayEnabled && e.CloseReason == CloseReason.UserClosing && _tray != null)
            {
                e.Cancel = true;
                HideToTray(false);
                return;
            }
            _tickTimer.Stop();
            if (_guardTimer != null) _guardTimer.Stop();
            _toastTimer.Stop();
            if (_tray != null)
            {
                _tray.Visible = false;
                _tray.Dispose();
                _tray = null;
            }
            try
            {
                _shell.Config.SetInt("WindowW", Width);
                _shell.Config.SetInt("WindowH", Height);
                if (WindowState == FormWindowState.Normal && string.IsNullOrEmpty(AppInfo.ShotPath))
                {
                    _shell.Config.SetInt("WindowX", Left);
                    _shell.Config.SetInt("WindowY", Top);
                }
                _shell.Config.SetBool("Maximized", WindowState == FormWindowState.Maximized);
                _shell.Config.Save();
            }
            catch { }
            base.OnFormClosing(e);
        }

        private void BuildChrome()
        {
            _sidebar = new Panel();
            _sidebar.BackColor = Theme.Sidebar;
            _sidebar.Paint += delegate(object s, PaintEventArgs e)
            {
                using (SolidBrush b = new SolidBrush(Theme.Sidebar))
                {
                    e.Graphics.FillRectangle(b, _sidebar.ClientRectangle);
                }
                using (Pen p = new Pen(Theme.Border))
                {
                    e.Graphics.DrawLine(p, _sidebar.Width - 1, 0, _sidebar.Width - 1, _sidebar.Height);
                }
            };
            Controls.Add(_sidebar);

            _content = new Panel();
            _content.BackColor = Theme.Bg;
            Controls.Add(_content);

            // На маленьком размере лучше читается иконка в рамке (замок без рамки используем крупно)
            _logoBox = new Panel();
            _logoBox.Size = new Size(38, 38);
            _logoBox.Location = new Point(18, 20);
            _logoBox.BackColor = Color.Transparent;
            _logoBox.Paint += delegate(object s, PaintEventArgs e)
            {
                e.Graphics.SmoothingMode = SmoothingMode.AntiAlias;
                Brand.DrawIcon(e.Graphics, new Rectangle(0, 0, 37, 37));
            };
            _sidebar.Controls.Add(_logoBox);

            _sidebar.Controls.Add(L("Zapret Manager", Theme.Bold, Theme.Text, 64, 24));
            _sidebar.Controls.Add(L("v" + AppInfo.Version, Theme.Small, Theme.TextMuted, 64, 42));
        }

        /// <summary>Ручная раскладка: сайдбар слева, контент справа, строка состояния снизу.</summary>
        /// <summary>Растягивает сайдбар, заголовок и статусбар под DPI (после постройки).</summary>
        private void ScaleChrome()
        {
            if (_chromeScaled) return;
            _chromeScaled = true;
            Theme.ScaleKeepingFonts(_sidebar, Theme.S);   // ширина сайдбара тоже увеличится
            Theme.ScaleKeepingFonts(_statusBar, Theme.S);
            if (_titleLbl != null) Theme.ScaleKeepingFonts(_titleLbl, Theme.S);
            if (_subtitleLbl != null) Theme.ScaleKeepingFonts(_subtitleLbl, Theme.S);
        }

        private void LayoutChrome()
        {
            if (_sidebar == null || _content == null || _statusBar == null) return;
            int w = ClientSize.Width;
            int h = ClientSize.Height;
            int statusH = 30;
            _sidebar.Bounds = new Rectangle(0, 0, SidebarWidth, h - statusH);
            _statusBar.Bounds = new Rectangle(0, h - statusH, w, statusH);
            _content.Bounds = new Rectangle(SidebarWidth, 0, Math.Max(100, w - SidebarWidth), Math.Max(100, h - statusH));
            LayoutContent();
            _sidebar.Invalidate();
        }

        private void BuildSidebarBase()
        {
            _navGroup1 = L("УПРАВЛЕНИЕ", Theme.Small, Theme.TextMuted, 24, 76);
            _sidebar.Controls.Add(_navGroup1);
        }

        private void BuildContent()
        {
            _titleLbl = new Label();
            _titleLbl.Font = Theme.Title;
            _titleLbl.ForeColor = Theme.Text;
            _titleLbl.AutoSize = true;
            _titleLbl.Location = new Point(24, 18);
            _titleLbl.BackColor = Color.Transparent;
            _content.Controls.Add(_titleLbl);

            _subtitleLbl = new Label();
            _subtitleLbl.Font = Theme.Base;
            _subtitleLbl.ForeColor = Theme.TextDim;
            _subtitleLbl.AutoSize = true;
            _subtitleLbl.Location = new Point(26, 52);
            _subtitleLbl.BackColor = Color.Transparent;
            _content.Controls.Add(_subtitleLbl);

            _pageHost = new Panel();
            _pageHost.Location = new Point(24, 88);
            _pageHost.BackColor = Theme.Bg;
            _content.Controls.Add(_pageHost);

            LayoutContent();
        }

        private void LayoutContent()
        {
            if (_pageHost == null || _content == null) return;
            _titleLbl.Location = new Point(24, 18);
            _subtitleLbl.Location = new Point(26, 52);
            _pageHost.Location = new Point(24, 88);
            _pageHost.Size = new Size(Math.Max(200, _content.ClientSize.Width - 48), Math.Max(100, _content.ClientSize.Height - 88 - 24));
        }

        private void BuildStatusBar()
        {
            _statusBar = new Panel();
            _statusBar.Height = 30;
            _statusBar.BackColor = Theme.Sidebar;
            _statusBar.Paint += delegate(object s, PaintEventArgs e)
            {
                using (Pen p = new Pen(Theme.Border))
                {
                    e.Graphics.DrawLine(p, 0, 0, _statusBar.Width, 0);
                }
            };
            Controls.Add(_statusBar);

            _dotBypass = new StatusDot();
            _dotBypass.Location = new Point(16, 10);
            _statusBar.Controls.Add(_dotBypass);
            _lblBypass = L("Bypass: —", Theme.Small, Theme.TextDim, 32, 6);
            _statusBar.Controls.Add(_lblBypass);

            _dotService = new StatusDot();
            _dotService.Location = new Point(176, 10);
            _statusBar.Controls.Add(_dotService);
            _lblService = L("Служба: —", Theme.Small, Theme.TextDim, 192, 6);
            _statusBar.Controls.Add(_lblService);

            _lblIpSet = L("IPSet: —", Theme.Small, Theme.TextDim, 306, 6);
            _statusBar.Controls.Add(_lblIpSet);

            _statusRight = new Label();
            _statusRight.Font = Theme.Small;
            _statusRight.ForeColor = Theme.TextMuted;
            _statusRight.AutoSize = true;
            _statusRight.BackColor = Color.Transparent;
            _statusRight.Text = "";
            _statusBar.Controls.Add(_statusRight);
            _statusBar.Resize += delegate { PositionStatusRight(); };
            PositionStatusRight();

            _statusLeft = new Label();
            _statusLeft.AutoSize = true;
            _statusLeft.Location = new Point(420, 6);
            _statusLeft.Font = Theme.Small;
            _statusLeft.ForeColor = Theme.TextMuted;
            _statusLeft.BackColor = Color.Transparent;
            _statusBar.Controls.Add(_statusLeft);
        }

        private void PositionStatusRight()
        {
            if (_statusRight == null) return;
            int w = TextRenderer.MeasureText(_statusRight.Text, _statusRight.Font).Width;
            _statusRight.Location = new Point(Math.Max(430, _statusBar.ClientSize.Width - w - 16), 6);
        }

        /// <summary>Переставляет элементы строки состояния слева направо без наложений.</summary>
        private void RelayoutStatus()
        {
            if (_lblBypass == null) return;
            int x = 16;
            _dotBypass.Location = new Point(x, 10);
            x += 16;
            _lblBypass.Location = new Point(x, 6);
            x += TextRenderer.MeasureText(_lblBypass.Text, _lblBypass.Font).Width + 18;
            _dotService.Location = new Point(x, 10);
            x += 16;
            _lblService.Location = new Point(x, 6);
            x += TextRenderer.MeasureText(_lblService.Text, _lblService.Font).Width + 18;
            _lblIpSet.Location = new Point(x, 6);
        }

        private void BuildToast()
        {
            _toast = new Panel();
            _toast.Size = new Size(430, 56);
            _toast.Visible = false;
            _toast.BackColor = Color.Transparent;
            _toast.Paint += delegate(object s, PaintEventArgs e)
            {
                e.Graphics.SmoothingMode = SmoothingMode.AntiAlias;
                Rectangle r = new Rectangle(0, 0, _toast.Width - 1, _toast.Height - 1);
                Draw.FillRound(e.Graphics, r, 10, Color.FromArgb(252, 30, 25, 20));
                Draw.DrawRound(e.Graphics, r, 10, Theme.BorderStrong, 1f);
                Draw.FillRound(e.Graphics, new Rectangle(0, 10, 3, _toast.Height - 20), 2, _toastAccent);
            };
            _toastLabel = new Label();
            _toastLabel.AutoSize = false;
            _toastLabel.Location = new Point(16, 10);
            _toastLabel.Size = new Size(398, 36);
            _toastLabel.BackColor = Color.Transparent;
            _toastLabel.ForeColor = Theme.Text;
            _toastLabel.Font = Theme.Base;
            _toast.Controls.Add(_toastLabel);
            Controls.Add(_toast);
            _toast.BringToFront();

            _toastTimer = new System.Windows.Forms.Timer();
            _toastTimer.Interval = 6000;
            _toastTimer.Tick += delegate { _toast.Visible = false; _toastTimer.Stop(); };
        }

        private Color _toastAccent = Theme.Accent;

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

        public void ShowToast(string message, LogLevel level)
        {
            _toastAccent = level == LogLevel.Ok ? Theme.Success : (level == LogLevel.Warn ? Theme.Warn : (level == LogLevel.Error ? Theme.Danger : Theme.Accent));
            _toastLabel.Text = message;
            _toast.Visible = true;
            _toast.BringToFront();
            _toastTimer.Stop();
            _toastTimer.Start();
            PositionToast();
        }

        private void PositionToast()
        {
            if (_toast == null) return;
            _toast.Location = new Point(ClientSize.Width - _toast.Width - 24, ClientSize.Height - _toast.Height - 46);
        }

        protected override void OnResize(EventArgs e)
        {
            base.OnResize(e);
            if (WindowState == FormWindowState.Minimized && AppInfo.ShotPath == null && !AppInfo.ExitRequested
                && _shell.Config.GetBool("MinimizeToTray", true) && _tray != null)
            {
                HideToTray(false);
            }
            LayoutChrome();
            PositionToast();
            PositionStatusRight();
        }

        protected override void OnMouseWheel(MouseEventArgs e)
        {
            if (_shell.Current != null) _shell.Current.View.Wheel(e.Delta);
            base.OnMouseWheel(e);
        }

        public void UpdateBusy()
        {
            Cursor = _shell.Busy ? Cursors.WaitCursor : Cursors.Default;
        }

        public void UpdateStatus()
        {
            RuntimeState st = _shell.State;
            if (st == null) return;
            if (st.WinwsRunning)
            {
                _dotBypass.SetColor(Theme.Success, true);
                string name = st.MatchedStrategy.Length > 0 ? st.MatchedStrategy : "пользовательская";
                _lblBypass.Text = "Bypass: работает — " + name;
            }
            else
            {
                _dotBypass.SetColor(Theme.TextMuted, false);
                _lblBypass.Text = "Bypass: не запущен";
            }
            _lblBypass.ForeColor = st.WinwsRunning ? Theme.Text : Theme.TextMuted;
            _dotService.SetColor(st.ServiceInstalled ? (st.ServiceStatus == "работает" ? Theme.Success : Theme.Warn) : Theme.TextMuted, false);
            _lblService.Text = "Служба: " + (st.ServiceInstalled ? st.ServiceStatus : "не установлена");
            _lblService.ForeColor = st.ServiceInstalled ? Theme.Text : Theme.TextMuted;
            _lblIpSet.Text = "IPSet: " + (st.IpsetMode == "loaded" ? "полный список" : (st.IpsetMode == "any" ? "любые IP" : "без фильтра"));
            RelayoutStatus();
            string right = string.Format("zapret {0} • {1}", _shell.LocalVersion, Native.IsAdmin() ? "администратор" : "без прав администратора");
            if (_statusRight.Text != right)
            {
                _statusRight.Text = right;
                PositionStatusRight();
            }
            _statusRight.ForeColor = Native.IsAdmin() ? Theme.TextMuted : Theme.Warn;
            LayoutContent();
        }

        public void SetStatusHint(string text)
        {
            _statusLeft.Text = text;
        }

        /// <summary>Размер окна под масштаб макета: 1116×820 в единицах макета.</summary>
        private void ApplyLayoutScale()
        {
            if (Theme.S <= 1.01f) return;
            Rectangle work = Screen.FromControl(this).WorkingArea;
            int width = Math.Min(Theme.Px(1120), work.Width - Theme.Px(20));
            int height = Math.Min(Theme.Px(860), work.Height - Theme.Px(20));
            Log.Info(string.Format("Масштаб макета: S={0:0.00}, клиент {1}x{2} → нужно {3}x{4} (экран {5}x{6})",
                Theme.S, ClientSize.Width, ClientSize.Height, width, height, work.Width, work.Height));
            if (ClientSize.Width < width || ClientSize.Height < height)
            {
                ClientSize = new Size(Math.Max(ClientSize.Width, width), Math.Max(ClientSize.Height, height));
                Log.Info("Масштаб макета: окно увеличено до " + ClientSize.Width + "x" + ClientSize.Height);
            }
        }

        public void RebuildPages()
        {
            string currentKey = _shell.Current != null ? _shell.Current.Key : "overview";
            foreach (PageBase p in _shell.Pages)
            {
                _pageHost.Controls.Remove(p.View);
                p.View.Dispose();
            }
            _shell.Pages.Clear();
            foreach (NavButton nb in _navButtons) nb.Dispose();
            _navButtons.Clear();
            _sidebar.Controls.Remove(_navGroup1);

            List<PageBase> pages = new List<PageBase>();
            pages.Add(new OverviewPage(_shell));
            pages.Add(new StrategiesPage(_shell));
            pages.Add(new TestPage(_shell));
            pages.Add(new SitesPage(_shell));
            pages.Add(new TgProxyPage(_shell));
            pages.Add(new SettingsPage(_shell));
            pages.Add(new ListsPage(_shell));
            pages.Add(new UpdatePage(_shell));
            pages.Add(new DiagnosticsPage(_shell));
            pages.Add(new ToolsPage(_shell));

            int y = 102;
            foreach (PageBase p in pages)
            {
                p.Build();
                Theme.ScaleKeepingFonts(p.View, Theme.S);
                p.View.Stack.PerformLayout();
                p.View.Stack.Height = Math.Max(10, p.View.Stack.ContentHeight);
                p.View.Visible = false;
                _pageHost.Controls.Add(p.View);
                _shell.Pages.Add(p);

                NavButton nb = new NavButton(p.Title, p.Icon);
                nb.Location = new Point(10, y);
                nb.Width = SidebarWidth - 20;
                nb.Tag = p.Key;
                nb.Click += delegate(object s, EventArgs e)
                {
                    NavButton b = (NavButton)s;
                    Navigate((string)b.Tag);
                };
                _sidebar.Controls.Add(nb);
                _navButtons.Add(nb);
                y += 44;
            }

            _shell.Current = null;
            Navigate(currentKey);
        }

        public void Navigate(string key)
        {
            PageBase target = null;
            foreach (PageBase p in _shell.Pages)
            {
                if (p.Key == key) { target = p; break; }
            }
            if (target == null && _shell.Pages.Count > 0) target = _shell.Pages[0];
            if (target == null) return;

            foreach (PageBase p in _shell.Pages) p.View.Visible = (p == target);
            foreach (NavButton nb in _navButtons) nb.Selected = string.Equals(Convert.ToString(nb.Tag), target.Key, StringComparison.Ordinal);
            foreach (NavButton nb in _navButtons) nb.Invalidate();

            _shell.Current = target;
            _titleLbl.Text = target.Title;
            _subtitleLbl.Text = target.Subtitle;
            target.OnShow();
            target.RefreshState();
        }

        /// <summary>Применяет тему (auto/dark/light) и перестраивает интерфейс.</summary>
        public void ApplyTheme(string mode)
        {
            Theme.Apply(mode);
            _shell.Config.Set("Theme", Theme.Mode);
            _shell.Config.Save();
            BackColor = Theme.Bg;
            _sidebar.BackColor = Theme.Sidebar;
            _content.BackColor = Theme.Bg;
            _statusBar.BackColor = Theme.Sidebar;
            _titleLbl.ForeColor = Theme.Text;
            _subtitleLbl.ForeColor = Theme.TextMuted;
            Native.SetTitleBarTheme(Handle, Theme.IsDark);
            RebuildPages();
            _shell.RefreshStateAsync();
            Log.Info("Оформление: " + Theme.Mode + " (" + (Theme.IsDark ? "тёмная" : "светлая") + ").");
        }

        // ---------- автопроверка обновлений и фоновый сторож сайтов ----------

        private System.Windows.Forms.Timer _autoCheckTimer;
        private System.Windows.Forms.Timer _guardTimer;
        private int _guardBusy;

        private void StartAutoCheckTimer()
        {
            if (_autoCheckTimer != null) return;
            _autoCheckTimer = new System.Windows.Forms.Timer();
            _autoCheckTimer.Interval = 20 * 60 * 1000;
            _autoCheckTimer.Tick += delegate { MaybeAutoCheck(); };
            _autoCheckTimer.Start();
        }

        private int AutoCheckHours()
        {
            int hours;
            if (!int.TryParse(_shell.Config.Get("AutoCheckHours", "6"), out hours)) hours = 6;
            if (hours < 1) hours = 1;
            return hours;
        }

        private void MaybeAutoCheck()
        {
            int hours = AutoCheckHours();
            DateTime last;
            if (DateTime.TryParse(_shell.Config.Get("LastAutoUpdateCheck", ""), out last)
                && (DateTime.Now - last).TotalHours < hours) return;

            _shell.Config.Set("LastAutoUpdateCheck", DateTime.Now.ToString("yyyy-MM-dd HH:mm:ss"));
            _shell.Config.Save();
            Log.Info("Автопроверка обновлений (раз в " + hours + " ч).");

            _shell.RunBackground(delegate()
            {
                SelfUpdateInfo self = SelfUpdate.Check(true);
                _shell.Ui(delegate()
                {
                    if (self.Ok && self.Available)
                    {
                        ShowToast("Доступна версия Zapret Manager " + self.Remote, LogLevel.Warn);
                        if (_tray != null)
                        {
                            try { _tray.ShowBalloonTip(5000, "Zapret Manager", "Доступна новая версия " + self.Remote + ".", ToolTipIcon.Info); } catch { }
                        }
                    }
                });
            }, null);

            if (!_shell.Install.IsValid) return;
            string local = _shell.LocalVersion;
            _shell.RunBackground(delegate()
            {
                UpdateCheckResult res = UpdateChecker.Check(local, true);
                _shell.Ui(delegate()
                {
                    _shell.UpdateState = res;
                    _shell.RaiseUpdateStateChanged();
                    if (res.Ok && res.UpdateAvailable)
                    {
                        ShowToast(string.Format("Доступно обновление zapret {0} → {1}", local, res.RemoteVersion), LogLevel.Warn);
                        if (_tray != null)
                        {
                            try { _tray.ShowBalloonTip(5000, "zapret", "Доступна версия " + res.RemoteVersion + ".", ToolTipIcon.Info); } catch { }
                        }
                    }
                });
            }, null);
        }

        /// <summary>Фоновый сторож: читает журнал DNS и добавляет заблокированные домены в списки.</summary>
        private void StartSiteGuard()
        {
            if (_shell.Config.GetBool("SiteGuard", false))
            {
                SiteGuard.EnableLog();
                SiteGuard.ResetCursor();
                Log.Info("Сторож сайтов включён: следим за запросами браузеров.");
            }
            if (_guardTimer != null) return;
            _guardTimer = new System.Windows.Forms.Timer();
            _guardTimer.Interval = 6000;
            _guardTimer.Tick += delegate { PollSiteGuard(); };
            _guardTimer.Start();
        }

        private void PollSiteGuard()
        {
            if (!_shell.Config.GetBool("SiteGuard", false)) return;
            if (_shell.Busy || _shell.Install == null || !_shell.Install.IsValid) return;
            if (System.Threading.Interlocked.CompareExchange(ref _guardBusy, 1, 0) != 0) return;

            ZapretInstall install = _shell.Install;
            bool autoAdd = _shell.Config.GetBool("SiteGuardAutoAdd", true);
            _shell.RunBackground(delegate()
            {
                try
                {
                    string error;
                    int scanned;
                    System.Collections.Generic.List<string> hosts = SiteGuard.NewBrowserHosts(out error, out scanned);
                    if (error.Length > 0 || hosts.Count == 0) return;
                    System.Collections.Generic.List<string> added = SiteGuard.Process(install, _shell.Config, hosts, autoAdd, delegate(string message)
                    {
                        _shell.Ui(delegate()
                        {
                            ShowToast("Сторож сайтов: " + message, LogLevel.Warn);
                            if (_tray != null)
                            {
                                try { _tray.ShowBalloonTip(5000, "Сторож сайтов", message, ToolTipIcon.Warning); } catch { }
                            }
                        });
                    });
                    if (added.Count > 0)
                    {
                        SiteGuard.Trim();
                        _shell.Ui(delegate() { _shell.RefreshState(true); });
                    }
                }
                catch (Exception ex)
                {
                    Log.Warn("Сторож сайтов: " + ex.Message);
                }
                finally
                {
                    System.Threading.Interlocked.Exchange(ref _guardBusy, 0);
                }
            }, null);
        }

        private void StartUpdateCheck()
        {
            if (!_shell.Config.GetBool("CheckUpdatesOnStartup", true)) return;
            StartAutoCheckTimer();
            _shell.Config.Set("LastAutoUpdateCheck", DateTime.Now.ToString("yyyy-MM-dd HH:mm:ss"));
            _shell.Config.Save();

            // обновление самого менеджера из репозитория
            _shell.RunBackground(delegate()
            {
                SelfUpdateInfo self = SelfUpdate.Check(true);
                _shell.Ui(delegate()
                {
                    if (self.Ok && self.Available)
                    {
                        ShowToast("Доступна версия Zapret Manager " + self.Remote, LogLevel.Warn);
                        Log.Warn("Доступно обновление менеджера " + self.Remote + " — страница «Обновление».");
                        if (_tray != null)
                        {
                            try { _tray.ShowBalloonTip(5000, "Zapret Manager", "Доступна новая версия " + self.Remote + ". Откройте «Обновление».", ToolTipIcon.Info); } catch { }
                        }
                    }
                    else if (!self.Ok)
                    {
                        Log.Info("Проверка обновления менеджера: " + self.Error);
                    }
                });
            }, null);

            if (!_shell.Install.IsValid) return;
            string local = _shell.LocalVersion;
            _shell.RunBackground(delegate()
            {
                UpdateCheckResult res = UpdateChecker.Check(local, true);
                _shell.Ui(delegate()
                {
                    _shell.UpdateState = res;
                    _shell.RaiseUpdateStateChanged();
                    if (res.Ok && res.UpdateAvailable)
                    {
                        ShowToast(string.Format("Доступно обновление zapret {0} → {1}", local, res.RemoteVersion), LogLevel.Warn);
                    }
                    else if (res.Ok)
                    {
                        Log.Ok("Обновлений нет, установлена последняя версия " + local + ".");
                    }
                    else
                    {
                        Log.Warn("Не удалось проверить обновления: " + res.Error);
                    }
                });
            }, null);
        }
    }

    // AppInfo объявлен в Core.cs
}
