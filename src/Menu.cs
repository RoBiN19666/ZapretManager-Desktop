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

    /// <summary>
    /// Тёмное контекстное меню в стиле приложения: скруглённые углы, тонкая рамка,
    /// подсветка строки «пилюлей» и простые значки слева.
    /// </summary>
    public sealed class DarkMenu : ContextMenuStrip
    {
        public const int Radius = 12;

        public DarkMenu()
        {
            Renderer = new DarkMenuRenderer();
            ShowImageMargin = false;
            ShowCheckMargin = false;
            Font = Theme.Base;
            BackColor = Theme.Surface;
            ForeColor = Theme.Text;
            Padding = new Padding(Theme.Px(8), Theme.Px(7), Theme.Px(8), Theme.Px(7));
            DropShadowEnabled = true;
        }

        protected override void OnResize(EventArgs e)
        {
            base.OnResize(e);
            try
            {
                using (GraphicsPath path = Shape.Round(new Rectangle(0, 0, Width, Height), Theme.Px(Radius)))
                {
                    Region = new Region(path);
                }
            }
            catch { }
        }
    }

    /// <summary>Отрисовка пунктов меню: подсветка, текст, значки, разделители.</summary>
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

            string kind = e.Item.Tag as string;
            if (!string.IsNullOrEmpty(kind))
            {
                Color glyph = (e.Item.Selected || e.Item.Pressed) ? Theme.Accent : Theme.TextDim;
                Rectangle box = new Rectangle(e.Item.Bounds.Left + Theme.Px(10),
                    e.Item.Height / 2 - Theme.Px(8), Theme.Px(16), Theme.Px(16));
                MenuGlyph.Draw(e.Graphics, kind, box, glyph);
            }

            Rectangle text = e.TextRectangle;
            text.X = e.Item.Bounds.Left + Theme.Px(36);
            text.Width = Math.Max(Theme.Px(40), e.Item.Width - Theme.Px(50));
            e.TextRectangle = text;
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

        protected override void OnRenderItemImage(ToolStripItemImageRenderEventArgs e)
        {
        }

        protected override void OnRenderItemCheck(ToolStripItemImageRenderEventArgs e)
        {
        }
    }

    /// <summary>Простые значки для пунктов меню (рисуются линиями, без шрифтов).</summary>
    public static class MenuGlyph
    {
        public static void Draw(Graphics g, string kind, Rectangle box, Color color)
        {
            g.SmoothingMode = SmoothingMode.AntiAlias;
            int cx = box.X + box.Width / 2;
            int cy = box.Y + box.Height / 2;
            int s = Math.Min(box.Width, box.Height);
            using (Pen p = new Pen(color, Math.Max(1.4f, s / 11f)))
            {
                p.StartCap = LineCap.Round;
                p.EndCap = LineCap.Round;
                switch (kind)
                {
                    case "open":
                        g.DrawRectangle(p, cx - s / 4, cy - s / 4, s / 2, s / 2);
                        g.DrawLine(p, cx - s / 4, cy - s / 4, cx + s / 5, cy + s / 5);
                        break;
                    case "shield":
                        g.DrawArc(p, cx - s / 3, cy - s / 3, s * 2 / 3, s * 2 / 3, 200, 340);
                        g.DrawLine(p, cx - s / 3, cy - s / 3, cx + s / 5, cy - s / 3);
                        break;
                    case "telegram":
                        g.DrawLines(p, new Point[]
                        {
                            new Point(cx - s / 3, cy),
                            new Point(cx + s / 3, cy - s / 4),
                            new Point(cx - s / 10, cy + s / 3),
                            new Point(cx - s / 10, cy + s / 8)
                        });
                        break;
                    case "refresh":
                        g.DrawArc(p, cx - s / 3, cy - s / 3, s * 2 / 3, s * 2 / 3, 40, 280);
                        g.DrawLine(p, cx + s / 5, cy - s / 3, cx + s / 12, cy - s / 6);
                        break;
                    case "exit":
                        g.DrawArc(p, cx - s / 3, cy - s / 3, s * 2 / 3, s * 2 / 3, 110, 320);
                        g.DrawLine(p, cx, cy - s / 3, cx, cy);
                        break;
                }
            }
        }
    }

    /// <summary>Служебный предпросмотр меню для скриншотов (--menu-shot).</summary>
    public static class MenuPreview
    {
        public static void Run(string path)
        {
            DarkMenu menu = new DarkMenu();
            string[] titles = new string[] { "Открыть Zapret Manager", "Запрет (обход DPI)", "TG WS Proxy", "-", "Проверить обновления", "-", "Выход" };
            string[] kinds = new string[] { "open", "shield", "telegram", "", "refresh", "", "exit" };
            for (int i = 0; i < titles.Length; i++)
            {
                if (titles[i] == "-") { menu.Items.Add(new ToolStripSeparator()); continue; }
                ToolStripMenuItem item = new ToolStripMenuItem(titles[i]);
                item.Font = Theme.Base;
                item.Padding = new Padding(Theme.Px(34), Theme.Px(7), Theme.Px(18), Theme.Px(7));
                item.Tag = kinds[i];
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
