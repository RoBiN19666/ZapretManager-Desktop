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

    /// <summary>Значки пунктов меню из Segoe MDL2 Assets: ровные, штатного размера.</summary>
    public static class MenuGlyph
    {
        private static readonly System.Collections.Generic.Dictionary<int, Font> Fonts = new System.Collections.Generic.Dictionary<int, Font>();

        private static string Code(string kind)
        {
            switch (kind)
            {
                case "open": return "\uE8A7";      // окно со стрелкой
                case "shield": return "\uEA18";    // щит
                case "telegram": return "\uE724";   // отправить (самолётик)
                case "refresh": return "\uE72C";   // круговая стрелка
                case "exit": return "\uE7E8";      // питание
                default: return "";
            }
        }

        public static void Draw(Graphics g, string kind, Rectangle box, Color color)
        {
            string code = Code(kind);
            if (code.Length == 0) return;
            int size = Math.Max(10, box.Height);
            Font font;
            if (!Fonts.TryGetValue(size, out font))
            {
                try
                {
                    font = new Font("Segoe MDL2 Assets", size, FontStyle.Regular, GraphicsUnit.Pixel);
                    Fonts[size] = font;
                }
                catch
                {
                    return;
                }
            }
            TextRenderer.DrawText(g, code, font, box, color,
                TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter | TextFormatFlags.NoPrefix);
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

    /// <summary>Служебный тест: таблица глифов Segoe MDL2/Fluent для подбора значков.</summary>
    public static class GlyphTest
    {
        public static void Run(string path)
        {
            int[] codes = new int[]
            {
                0xE700, 0xE713, 0xE72C, 0xE7E8, 0xE8A7, 0xE8AB, 0xE8B7, 0xE8C8,
                0xE71A, 0xE768, 0xE769, 0xEA18, 0xE83D, 0xE72E, 0xE8F1, 0xE8F2,
                0xE8FB, 0xE9D9, 0xE7BA, 0xE8A1, 0xE71B, 0xE777, 0xE778, 0xE7EE
            };
            Font font = new Font("Segoe MDL2 Assets", 22f, FontStyle.Regular, GraphicsUnit.Point);
            Font small = new Font("Consolas", 10f);
            int cell = 90, cols = 6;
            int rows = (codes.Length + cols - 1) / cols;
            using (Bitmap bmp = new Bitmap(cols * cell, rows * cell))
            using (Graphics g = Graphics.FromImage(bmp))
            {
                g.Clear(Color.FromArgb(26, 22, 20));
                g.TextRenderingHint = System.Drawing.Text.TextRenderingHint.ClearTypeGridFit;
                for (int i = 0; i < codes.Length; i++)
                {
                    int x = (i % cols) * cell, y = (i / cols) * cell;
                    using (SolidBrush b = new SolidBrush(Color.FromArgb(244, 238, 231)))
                    {
                        g.DrawString(((char)codes[i]).ToString(), font, b, x + 24, y + 14);
                    }
                    using (SolidBrush b = new SolidBrush(Color.FromArgb(255, 194, 74)))
                    {
                        g.DrawString(codes[i].ToString("X4"), small, b, x + 26, y + 62);
                    }
                }
                bmp.Save(path, System.Drawing.Imaging.ImageFormat.Png);
            }
            Log.Info("Таблица глифов: " + path);
        }
    }
}
