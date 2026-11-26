using System;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Windows.Forms;

namespace ZapretManager
{
    /// <summary>Скруглённый путь для меню и подсветки.</summary>
    public static class Shape
    {
        public static GraphicsPath Round(Rectangle r, int radius)
        {
            GraphicsPath path = new GraphicsPath();
            int d = Math.Max(2, radius * 2);
            if (d > r.Width) d = r.Width;
            if (d > r.Height) d = r.Height;
            path.AddArc(r.X, r.Y, d, d, 180, 90);
            path.AddArc(r.Right - d, r.Y, d, d, 270, 90);
            path.AddArc(r.Right - d, r.Bottom - d, d, d, 0, 90);
            path.AddArc(r.X, r.Bottom - d, d, d, 90, 90);
            path.CloseFigure();
            return path;
        }
    }

    /// <summary>Иконки пунктов меню: глиф Segoe MDL2, отрисованный в картинку.</summary>
    public static class MenuIcons
    {
        private static readonly System.Collections.Generic.Dictionary<string, Image> Cache =
            new System.Collections.Generic.Dictionary<string, Image>();

        public static string Code(string kind)
        {
            switch (kind)
            {
                case "open": return "\uE8A7";
                case "shield": return "\uEA18";
                case "telegram": return "\uE724";
                case "refresh": return "\uE72C";
                case "exit": return "\uE7E8";
                default: return "";
            }
        }

        public static Image Get(string kind)
        {
            string code = Code(kind);
            if (code.Length == 0) return null;
            Image cached;
            if (Cache.TryGetValue(kind, out cached)) return cached;
            try
            {
                int size = Theme.Px(18);
                Bitmap bmp = new Bitmap(size, size);
                using (Graphics g = Graphics.FromImage(bmp))
                using (Font font = new Font("Segoe MDL2 Assets", size, FontStyle.Regular, GraphicsUnit.Pixel))
                using (SolidBrush brush = new SolidBrush(Theme.TextDim))
                using (StringFormat format = new StringFormat())
                {
                    format.Alignment = StringAlignment.Center;
                    format.LineAlignment = StringAlignment.Center;
                    g.TextRenderingHint = System.Drawing.Text.TextRenderingHint.ClearTypeGridFit;
                    g.DrawString(code, font, brush, new RectangleF(0, 0, size, size), format);
                }
                Cache[kind] = bmp;
                return bmp;
            }
            catch
            {
                return null;
            }
        }
    }

    /// <summary>
    /// Тёмное контекстное меню в стиле приложения: скруглённые углы, тонкая рамка,
    /// подсветка строки «пилюлей», штатные иконки и текст по центру строки.
    /// </summary>
    public sealed class DarkMenu : ContextMenuStrip
    {
        public const int Radius = 12;

        public DarkMenu()
        {
            Renderer = new DarkMenuRenderer();
            ShowImageMargin = true;
            ShowCheckMargin = false;
            Font = Theme.Base;
            BackColor = Theme.Surface;
            ForeColor = Theme.Text;
            Padding = new Padding(Theme.Px(6), Theme.Px(7), Theme.Px(8), Theme.Px(7));
            DropShadowEnabled = true;
        }

        /// <summary>Единое оформление пункта: шрифт, отступы, иконка.</summary>
        public static void StyleItem(ToolStripItem item, string kind)
        {
            item.Font = Theme.Base;
            item.Padding = new Padding(Theme.Px(2), Theme.Px(7), Theme.Px(18), Theme.Px(7));
            item.ImageScaling = ToolStripItemImageScaling.None;
            if (!string.IsNullOrEmpty(kind)) item.Image = MenuIcons.Get(kind);
        }
        protected override void OnResize(EventArgs e)
        {
            base.OnResize(e);
            try
            {
                Region old = Region;
                using (GraphicsPath path = Shape.Round(new Rectangle(0, 0, Width, Height), Theme.Px(Radius)))
                {
                    Region = new Region(path);
                }
                if (old != null) old.Dispose();
            }
            catch { }
        }
    }

    /// <summary>Отрисовка: фон и рамка со скруглением, подсветка строки, разделители.</summary>
    public sealed class DarkMenuRenderer : ToolStripRenderer
    {
        protected override void OnRenderToolStripBackground(ToolStripRenderEventArgs e)
        {
            Graphics g = e.Graphics;
            g.SmoothingMode = SmoothingMode.AntiAlias;
            Rectangle r = new Rectangle(0, 0, e.ToolStrip.Width - 1, e.ToolStrip.Height - 1);
            using (GraphicsPath path = Shape.Round(r, Theme.Px(DarkMenu.Radius)))
            {
                using (SolidBrush b = new SolidBrush(Theme.Surface)) g.FillPath(b, path);
                using (Pen p = new Pen(Theme.BorderStrong)) g.DrawPath(p, path);
            }
        }

        protected override void OnRenderToolStripBorder(ToolStripRenderEventArgs e)
        {
        }

        protected override void OnRenderMenuItemBackground(ToolStripItemRenderEventArgs e)
        {
            ToolStripItem item = e.Item;
            if (!item.Selected && !item.Pressed) return;
            Graphics g = e.Graphics;
            g.SmoothingMode = SmoothingMode.AntiAlias;
            Rectangle r = new Rectangle(Theme.Px(4), 1, item.Width - Theme.Px(8), item.Height - 2);
            using (GraphicsPath path = Shape.Round(r, Theme.Px(7)))
            using (SolidBrush b = new SolidBrush(Theme.Hover)) g.FillPath(b, path);
        }

        protected override void OnRenderItemText(ToolStripItemTextRenderEventArgs e)
        {
            e.TextColor = e.Item.Enabled ? Theme.Text : Theme.TextMuted;
            base.OnRenderItemText(e);
        }

        protected override void OnRenderSeparator(ToolStripSeparatorRenderEventArgs e)
        {
            Graphics g = e.Graphics;
            int y = e.Item.Height / 2;
            using (Pen p = new Pen(Theme.Border))
            {
                g.DrawLine(p, Theme.Px(12), y, e.Item.Width - Theme.Px(12), y);
            }
        }
    }

    /// <summary>Служебный предпросмотр меню для скриншотов (--menu-shot).</summary>
    public static class MenuPreview
    {
        public static void RunAtCursor(string path)
        {
            System.Drawing.Point at = Native.CursorPosition();
            using (Form host = new Form())
            {
                host.ShowInTaskbar = false;
                host.FormBorderStyle = FormBorderStyle.None;
                host.StartPosition = FormStartPosition.Manual;
                host.Location = new System.Drawing.Point(80, 80);
                host.Size = new Size(400, 300);
                host.Show();
                host.Hide();
                Log.Info("Проба: окно скрыто, visible=" + host.Visible);

                DarkMenu menu = new DarkMenu();
                menu.Items.Add(new ToolStripMenuItem("Проба"));
                menu.Show(at);
                Native.SetForegroundWindow(menu.Handle);
                Application.DoEvents();
                Application.DoEvents();
                Log.Info("Проба: после меню visible=" + host.Visible + ", state=" + host.WindowState + ", меню " + menu.Bounds);

                host.BeginInvoke((MethodInvoker)delegate { if (host.Visible) { host.Hide(); host.ShowInTaskbar = false; } });
                Application.DoEvents();
                Application.DoEvents();
                Log.Info("Проба: после скрытия visible=" + host.Visible);
                menu.Close();
            }
        }


        public static void Run(string path)
        {
            DarkMenu menu = new DarkMenu();
            string[] titles = new string[] { "Открыть Zapret Manager", "Запрет (обход DPI)", "TG WS Proxy", "-", "Проверить обновления", "-", "Выход" };
            string[] kinds = new string[] { "open", "shield", "telegram", "", "refresh", "", "exit" };
            for (int i = 0; i < titles.Length; i++)
            {
                if (titles[i] == "-") { menu.Items.Add(new ToolStripSeparator()); continue; }
                ToolStripMenuItem item = new ToolStripMenuItem(titles[i]);
                DarkMenu.StyleItem(item, kinds[i]);
                menu.Items.Add(item);
            }

            using (Timer timer = new Timer())
            {
                timer.Interval = 800;
                timer.Tick += delegate
                {
                    timer.Stop();
                    Capture(menu, path);
                    menu.Close();
                    Application.ExitThread();
                };
                timer.Start();
                menu.Show(new Point(Theme.Px(40), Theme.Px(40)));
                Application.Run();
            }
        }

        private static void Capture(DarkMenu menu, string path)
        {
            try
            {
                Rectangle r = menu.Bounds;
                using (Bitmap bmp = new Bitmap(r.Width + 24, r.Height + 24))
                {
                    using (Graphics g = Graphics.FromImage(bmp))
                    {
                        g.Clear(Theme.Bg);
                        g.CopyFromScreen(r.Left - 12, r.Top - 12, 0, 0, new Size(r.Width + 24, r.Height + 24));
                    }
                    bmp.Save(path, System.Drawing.Imaging.ImageFormat.Png);
                }
                Log.Info("Снимок меню: " + path);
            }
            catch (Exception ex)
            {
                Log.Warn("Снимок меню: " + ex.Message);
            }
        }
    }
}
