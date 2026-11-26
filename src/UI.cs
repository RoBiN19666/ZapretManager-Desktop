using System;
using System.Collections.Generic;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Drawing.Text;
using System.Windows.Forms;

namespace ZapretManager
{
    public static class Theme
    {
        // Палитра под логотип: тёплый чёрный + оранжево-золотой акцент.
        public static Color Bg = Color.FromArgb(11, 10, 10);
        public static Color Sidebar = Color.FromArgb(16, 14, 13);
        public static Color SidebarTop = Color.FromArgb(18, 16, 14);
        public static Color SidebarBottom = Color.FromArgb(18, 16, 14);
        public static Color Surface = Color.FromArgb(26, 22, 20);
        public static Color SurfaceTop = Surface;
        public static Color SurfaceBottom = Surface;
        public static Color SurfaceAlt = Color.FromArgb(38, 32, 27);
        public static Color Hover = Color.FromArgb(50, 43, 36);
        public static Color Border = Color.FromArgb(52, 44, 37);
        public static Color BorderStrong = Color.FromArgb(78, 66, 54);
        public static Color Text = Color.FromArgb(244, 238, 231);
        public static Color TextDim = Color.FromArgb(178, 167, 155);
        public static Color TextMuted = Color.FromArgb(126, 116, 105);

        public static Color Accent = Color.FromArgb(249, 146, 56);
        public static Color AccentHover = Color.FromArgb(255, 172, 96);
        public static Color AccentPressed = Color.FromArgb(226, 116, 26);
        public static Color AccentTop = Color.FromArgb(245, 146, 63);
        public static Color AccentBottom = Color.FromArgb(245, 146, 63);
        public static Color AccentDeep = Color.FromArgb(200, 92, 14);
        public static Color OnAccent = Color.FromArgb(46, 25, 8);
        public static Color Selection = Color.FromArgb(46, 37, 30);

        public static Color Success = Color.FromArgb(79, 208, 138);
        public static Color Warn = Color.FromArgb(255, 194, 74);
        public static Color Danger = Color.FromArgb(255, 97, 82);

        /// <summary>Масштаб шрифтов относительно 96 DPI: при 125% шрифты уменьшаются, чтобы
        /// не расползаться по фиксированной пиксельной сетке макета.</summary>
        private static float _fontScale = 1f;

        public static Font Base = new Font("Segoe UI", 9.5f, FontStyle.Regular, GraphicsUnit.Point);
        public static Font Small = new Font("Segoe UI", 8.5f, FontStyle.Regular, GraphicsUnit.Point);
        public static Font Bold = new Font("Segoe UI Semibold", 9.5f, FontStyle.Regular, GraphicsUnit.Point);
        public static Font Title = new Font("Segoe UI Semibold", 17f, FontStyle.Regular, GraphicsUnit.Point);
        public static Font H2 = new Font("Segoe UI Semibold", 12f, FontStyle.Regular, GraphicsUnit.Point);
        public static Font Card = new Font("Segoe UI Semibold", 10f, FontStyle.Regular, GraphicsUnit.Point);
        public static Font Big = new Font("Segoe UI Semibold", 22f, FontStyle.Regular, GraphicsUnit.Point);
        public static Font Mono = new Font("Consolas", 9f, FontStyle.Regular, GraphicsUnit.Point);
        public static Font Icons = new Font("Segoe MDL2 Assets", 11f, FontStyle.Regular, GraphicsUnit.Point);

        /// <summary>Подгоняет шрифты под DPI экрана: макет нарисован в пикселях 96 DPI.</summary>
        public static void InitFonts()
        {
            InitFonts(1f);
        }

        /// <summary>Шрифты под DPI и пользовательский масштаб интерфейса (1.0 = 100%).</summary>
        public static void InitFonts(float userScale)
        {
            float dpi = 96f;
            try
            {
                using (Graphics g = Graphics.FromHwnd(IntPtr.Zero)) dpi = g.DpiX;
            }
            catch { }
            InitFonts(userScale, dpi);
        }

        /// <summary>Шрифты под конкретный DPI (монитор, на котором реально открыто окно).</summary>
        public static void InitFonts(float userScale, float dpi)
        {
            if (userScale <= 0.2f) userScale = 1f;
            Log.Info(string.Format("DPI окна: {0:0} (масштаб Windows {1:0}%)", dpi, dpi / 96f * 100));
            if (dpi < 96f) dpi = 96f;
            // На HiDPI макет растягивается целиком (Theme.S), а шрифты остаются проектными:
            // 9.5pt на 125 % — это физически крупный, чёткий текст, как в системных приложениях.
            float dpiFactor = dpi / 96f;
            if (dpiFactor < 1f) dpiFactor = 1f;
            S = dpiFactor * userScale;
            _fontScale = 1f / userScale;
            if (_fontScale > 1.001f || _fontScale < 0.999f)
            {
                Font old = Base;
                Base = new Font("Segoe UI", 9.5f / _fontScale, FontStyle.Regular, GraphicsUnit.Point);
                Small = new Font("Segoe UI", 8.5f / _fontScale, FontStyle.Regular, GraphicsUnit.Point);
                Bold = new Font("Segoe UI Semibold", 9.5f / _fontScale, FontStyle.Regular, GraphicsUnit.Point);
                Title = new Font("Segoe UI Semibold", 17f / _fontScale, FontStyle.Regular, GraphicsUnit.Point);
                H2 = new Font("Segoe UI Semibold", 12f / _fontScale, FontStyle.Regular, GraphicsUnit.Point);
                Card = new Font("Segoe UI Semibold", 10f / _fontScale, FontStyle.Regular, GraphicsUnit.Point);
                Big = new Font("Segoe UI Semibold", 22f / _fontScale, FontStyle.Regular, GraphicsUnit.Point);
                Mono = new Font("Consolas", 9f / _fontScale, FontStyle.Regular, GraphicsUnit.Point);
                Icons = new Font("Segoe MDL2 Assets", 11f / _fontScale, FontStyle.Regular, GraphicsUnit.Point);
                if (old != null) old.Dispose();
            }
        }

        /// <summary>Тёмная или светлая палитра приложения.</summary>
        public static bool IsDark = true;

        /// <summary>Режим оформления: auto | dark | light.</summary>
        public static string Mode = "auto";

        /// <summary>Определяет системную тему (Параметры → Персонализация → Цвет приложений).</summary>
        public static bool SystemPrefersDark()
        {
            try
            {
                using (Microsoft.Win32.RegistryKey key = Microsoft.Win32.Registry.CurrentUser.OpenSubKey(
                    @"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"))
                {
                    object v = key == null ? null : key.GetValue("AppsUseLightTheme");
                    if (v is int) return (int)v == 0;
                }
            }
            catch { }
            return true;
        }

        /// <summary>Применяет палитру по режиму. Возвращает true, если тема тёмная.</summary>
        public static bool Apply(string mode)
        {
            Mode = string.Equals(mode, "dark", StringComparison.OrdinalIgnoreCase) ? "dark"
                 : string.Equals(mode, "light", StringComparison.OrdinalIgnoreCase) ? "light" : "auto";
            bool dark = Mode == "dark" || (Mode == "auto" && SystemPrefersDark());
            IsDark = dark;
            if (dark)
            {
                Bg = Color.FromArgb(11, 10, 10);
                Sidebar = Color.FromArgb(16, 14, 13);
                SidebarTop = Sidebar;
                SidebarBottom = Sidebar;
                Surface = Color.FromArgb(26, 22, 20);
                SurfaceTop = Surface;
                SurfaceBottom = Surface;
                SurfaceAlt = Color.FromArgb(38, 32, 27);
                Hover = Color.FromArgb(50, 43, 36);
                Border = Color.FromArgb(52, 44, 37);
                BorderStrong = Color.FromArgb(78, 66, 54);
                Text = Color.FromArgb(244, 238, 231);
                TextDim = Color.FromArgb(178, 167, 155);
                TextMuted = Color.FromArgb(126, 116, 105);
                Selection = Color.FromArgb(46, 37, 30);
                Danger = Color.FromArgb(255, 97, 82);
                Success = Color.FromArgb(79, 208, 138);
                Warn = Color.FromArgb(255, 194, 74);
            }
            else
            {
                Bg = Color.FromArgb(245, 243, 240);
                Sidebar = Color.FromArgb(237, 234, 230);
                SidebarTop = Sidebar;
                SidebarBottom = Sidebar;
                Surface = Color.FromArgb(255, 255, 255);
                SurfaceTop = Surface;
                SurfaceBottom = Surface;
                SurfaceAlt = Color.FromArgb(242, 239, 235);
                Hover = Color.FromArgb(232, 228, 222);
                Border = Color.FromArgb(221, 215, 207);
                BorderStrong = Color.FromArgb(198, 189, 178);
                Text = Color.FromArgb(30, 26, 22);
                TextDim = Color.FromArgb(87, 80, 74);
                TextMuted = Color.FromArgb(139, 130, 120);
                Selection = Color.FromArgb(243, 226, 207);
                Danger = Color.FromArgb(200, 52, 40);
                Success = Color.FromArgb(30, 140, 88);
                Warn = Color.FromArgb(170, 118, 20);
            }
            Accent = Color.FromArgb(249, 146, 56);
            AccentHover = Color.FromArgb(255, 172, 96);
            AccentPressed = Color.FromArgb(226, 116, 26);
            AccentTop = Color.FromArgb(245, 146, 63);
            AccentBottom = Color.FromArgb(245, 146, 63);
            AccentDeep = Color.FromArgb(200, 92, 14);
            OnAccent = Color.FromArgb(46, 25, 8);
            return dark;
        }
        /// <summary>Текущий масштаб шрифтов (для отладки и логов).</summary>
        public static float FontScale { get { return _fontScale; } }

        /// <summary>Масштаб макета под DPI: 1.0 при 96 DPI, 1.25 при 125 %.</summary>
        public static float S = 1f;

        /// <summary>Пиксели макета → физические пиксели.</summary>
        public static int Px(int design)
        {
            return (int)Math.Round(design * S);
        }

        /// <summary>Масштабирует дерево контролов, сохраняя размеры шрифтов (макет растёт, текст — нет).</summary>
        public static void ScaleKeepingFonts(Control root, float factor)
        {
            if (root == null || factor <= 0f || Math.Abs(factor - 1f) < 0.01f) return;
            System.Collections.Generic.Dictionary<Control, Font> fonts = new System.Collections.Generic.Dictionary<Control, Font>();
            CollectFonts(root, fonts);
            root.Scale(new SizeF(factor, factor));
            foreach (System.Collections.Generic.KeyValuePair<Control, Font> pair in fonts)
            {
                try { pair.Key.Font = pair.Value; } catch { }
            }
        }

        private static void CollectFonts(Control control, System.Collections.Generic.Dictionary<Control, Font> fonts)
        {
            fonts[control] = control.Font;
            foreach (Control child in control.Controls) CollectFonts(child, fonts);
        }

        public static Color WithAlpha(Color c, int alpha)
        {
            return Color.FromArgb(alpha, c.R, c.G, c.B);
        }
    }

    public static class Draw
    {
        public static GraphicsPath RoundRect(Rectangle r, int radius)
        {
            GraphicsPath p = new GraphicsPath();
            if (r.Width <= 0 || r.Height <= 0) { p.AddRectangle(r); return p; }
            int d = radius * 2;
            if (d <= 0 || d > r.Height) d = Math.Min(r.Height, r.Width);
            if (d <= 1) { p.AddRectangle(r); return p; }
            p.AddArc(r.X, r.Y, d, d, 180, 90);
            p.AddArc(r.Right - d, r.Y, d, d, 270, 90);
            p.AddArc(r.Right - d, r.Bottom - d, d, d, 0, 90);
            p.AddArc(r.X, r.Bottom - d, d, d, 90, 90);
            p.CloseFigure();
            return p;
        }

        public static void FillRound(Graphics g, Rectangle r, int radius, Color color)
        {
            using (GraphicsPath p = RoundRect(r, radius))
            using (SolidBrush b = new SolidBrush(color))
            {
                g.FillPath(b, p);
            }
        }

        /// <summary>Заливка сочным вертикальным градиентом (фирменный «глянец»).</summary>
        public static void FillRoundGradient(Graphics g, Rectangle r, int radius, Color top, Color bottom)
        {
            if (r.Width <= 0 || r.Height <= 0) return;
            using (GraphicsPath p = RoundRect(r, radius))
            using (LinearGradientBrush b = new LinearGradientBrush(
                new Rectangle(r.X, r.Y, Math.Max(1, r.Width), Math.Max(1, r.Height + 1)),
                top, bottom, LinearGradientMode.Vertical))
            {
                g.FillPath(b, p);
            }
        }

        /// <summary>Глянцевая подсветка сверху — как на логотипе.</summary>
        public static void Gloss(Graphics g, Rectangle r, int radius, int alpha)
        {
            if (r.Height < 6 || alpha <= 0) return;
            Rectangle half = new Rectangle(r.X + 1, r.Y + 1, r.Width - 2, Math.Max(2, (r.Height - 2) / 2));
            using (GraphicsPath p = RoundRect(half, Math.Max(2, radius - 1)))
            using (LinearGradientBrush b = new LinearGradientBrush(half, Color.FromArgb(alpha, 255, 255, 255), Color.FromArgb(0, 255, 255, 255), LinearGradientMode.Vertical))
            {
                g.FillPath(b, p);
            }
        }

        /// <summary>Тонкая светлая кромка сверху — аккуратный «объём» без глянца.</summary>
        public static void TopHighlight(Graphics g, Rectangle r, int radius)
        {
            if (r.Width < 4 || r.Height < 4) return;
            int d = Math.Max(2, Math.Min(radius, r.Height / 2) * 2);
            Rectangle top = new Rectangle(r.X + 1, r.Y + 1, r.Width - 2, d);
            if (top.Width <= d) return;
            using (GraphicsPath p = new GraphicsPath())
            {
                p.AddArc(top.X, top.Y, d, d, 180, 90);
                p.AddArc(top.Right - d, top.Y, d, d, 270, 90);
                using (Pen pen = new Pen(Color.FromArgb(64, 255, 255, 255), 1f))
                {
                    g.DrawPath(pen, p);
                }
            }
        }

        public static void DrawRound(Graphics g, Rectangle r, int radius, Color color, float width)
        {
            using (GraphicsPath p = RoundRect(r, radius))
            using (Pen pen = new Pen(color, width))
            {
                g.DrawPath(pen, p);
            }
        }

        public static void Text(Graphics g, string text, Font font, Color color, Rectangle bounds, TextFormatFlags flags)
        {
            TextRenderer.DrawText(g, text, font, bounds, color, flags);
        }

        public static readonly TextFormatFlags LeftMiddle = TextFormatFlags.Left | TextFormatFlags.VerticalCenter | TextFormatFlags.NoPrefix | TextFormatFlags.EndEllipsis;
        public static readonly TextFormatFlags Center = TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter | TextFormatFlags.NoPrefix;
    }

    /// <summary>Карточка-контейнер с заголовком.</summary>
    public class Card : Panel
    {
        public string Header = "";
        public Color AccentStripe = Color.Empty;
        public int Radius = 12;

        public Card() : this("", 120) { }

        public Card(string header, int height)
        {
            Header = header;
            DoubleBuffered = true;
            ResizeRedraw = true;
            BackColor = Theme.Surface;
            SetStyle(ControlStyles.SupportsTransparentBackColor, true);
            Height = height;
            Margin = new Padding(0, 0, 0, 12);
        }

        protected override void OnPaint(PaintEventArgs e)
        {
            Graphics g = e.Graphics;
            g.SmoothingMode = SmoothingMode.AntiAlias;
            Rectangle r = new Rectangle(0, 0, Width - 1, Height - 1);
            Draw.FillRound(g, r, Radius, Theme.Surface);
            Draw.DrawRound(g, r, Radius, Theme.Border, 1f);
            if (AccentStripe != Color.Empty)
            {
                Draw.FillRound(g, new Rectangle(1, 14, 3, Height - 28), 2, AccentStripe);
            }
            if (!string.IsNullOrEmpty(Header))
            {
                Draw.Text(g, Header, Theme.Card, Theme.Text, new Rectangle(18, 12, Width - 36, 22), TextFormatFlags.Left | TextFormatFlags.VerticalCenter | TextFormatFlags.NoPrefix);
            }
            base.OnPaint(e);
        }
    }

    public enum ButtonKind { Primary, Secondary, Ghost, Danger, Link }

    public class RoundButton : Control, IButtonControl
    {
        public ButtonKind Kind = ButtonKind.Secondary;
        public int Radius = 8;
        public string Glyph = "";
        private bool _hover;
        private bool _pressed;

        private DialogResult _dialogResult = DialogResult.None;

        public DialogResult DialogResult
        {
            get { return _dialogResult; }
            set { _dialogResult = value; }
        }

        public void NotifyDefault(bool value) { }

        public void PerformClick()
        {
            if (Enabled) OnClick(EventArgs.Empty);
        }

        public RoundButton()
        {
            SetStyle(ControlStyles.AllPaintingInWmPaint | ControlStyles.UserPaint | ControlStyles.OptimizedDoubleBuffer | ControlStyles.SupportsTransparentBackColor | ControlStyles.ResizeRedraw, true);
            BackColor = Color.Transparent;
            Font = Theme.Bold;
            Cursor = Cursors.Hand;
            Height = 32;
            Width = 100;
        }

        protected override void OnMouseEnter(EventArgs e) { _hover = true; Invalidate(); base.OnMouseEnter(e); }
        protected override void OnMouseLeave(EventArgs e) { _hover = false; _pressed = false; Invalidate(); base.OnMouseLeave(e); }
        protected override void OnMouseDown(MouseEventArgs e) { _pressed = true; Invalidate(); base.OnMouseDown(e); }
        protected override void OnMouseUp(MouseEventArgs e) { _pressed = false; Invalidate(); base.OnMouseUp(e); }
        protected override void OnEnabledChanged(EventArgs e) { Invalidate(); base.OnEnabledChanged(e); }

        protected override void OnPaint(PaintEventArgs e)
        {
            Graphics g = e.Graphics;
            g.SmoothingMode = SmoothingMode.AntiAlias;
            Rectangle r = new Rectangle(0, 0, Width - 1, Height - 1);
            Color fill = Color.Transparent, border = Color.Transparent, text = Theme.Text;
            Color gradTop = Color.Empty, gradBottom = Color.Empty;


            switch (Kind)
            {
                case ButtonKind.Primary:
                    if (!Enabled)
                    {
                        fill = Theme.SurfaceAlt;
                        border = Theme.Border;
                        text = Theme.TextMuted;
                    }
                    else
                    {
                        if (_pressed) fill = Theme.AccentPressed;
                        else if (_hover) fill = Theme.AccentHover;
                        else fill = Theme.Accent;
                        border = Theme.WithAlpha(Color.White, 18);
                        text = Theme.OnAccent;
                    }
                    break;
                case ButtonKind.Secondary:
                    fill = !Enabled ? Theme.Surface : (_pressed ? Theme.SurfaceAlt : (_hover ? Theme.Hover : Theme.SurfaceAlt));
                    border = _hover && Enabled ? Theme.BorderStrong : Theme.Border;
                    text = !Enabled ? Theme.TextMuted : Theme.Text;
                    break;
                case ButtonKind.Danger:
                    fill = !Enabled ? Theme.Surface : Theme.WithAlpha(Theme.Danger, _hover ? 30 : 16);
                    border = Theme.WithAlpha(Theme.Danger, _hover ? 105 : 66);
                    text = !Enabled ? Theme.TextMuted : Color.FromArgb(255, 143, 130);
                    break;
                case ButtonKind.Ghost:
                    fill = _hover ? Theme.WithAlpha(Color.White, 14) : Color.Transparent;
                    text = !Enabled ? Theme.TextMuted : (_hover ? Theme.Text : Theme.TextDim);
                    break;
                case ButtonKind.Link:
                    fill = Color.Transparent;
                    text = !Enabled ? Theme.TextMuted : (_hover ? Theme.AccentHover : Theme.Accent);
                    break;
            }

            if (fill != Color.Transparent) Draw.FillRound(g, r, Radius, fill);
            if (border != Color.Transparent) Draw.DrawRound(g, r, Radius, border, 1f);

            Rectangle textRect = ClientRectangle;
            if (!string.IsNullOrEmpty(Glyph) && GlyphSize() > 0)
            {
                int gw = GlyphWidth();
                Rectangle glyphRect = new Rectangle(10, 0, gw + 4, Height);
                TextRenderer.DrawText(g, Glyph, Theme.Icons, glyphRect, text, TextFormatFlags.Left | TextFormatFlags.VerticalCenter | TextFormatFlags.NoPrefix);
                textRect = new Rectangle(10 + gw + 8, 0, Width - gw - 24, Height);
                TextRenderer.DrawText(g, Text, Font, textRect, text, TextFormatFlags.Left | TextFormatFlags.VerticalCenter | TextFormatFlags.NoPrefix | TextFormatFlags.EndEllipsis);
            }
            else
            {
                TextRenderer.DrawText(g, Text, Font, textRect, text, TextFormatFlags.HorizontalCenter | TextFormatFlags.VerticalCenter | TextFormatFlags.NoPrefix | TextFormatFlags.EndEllipsis);
            }
        }

        private int GlyphWidth()
        {
            if (string.IsNullOrEmpty(Glyph)) return 0;
            return TextRenderer.MeasureText(Glyph, Theme.Icons).Width;
        }

        private int GlyphSize() { return string.IsNullOrEmpty(Glyph) ? 0 : 1; }

        /// <summary>Подгоняет ширину под текст.</summary>
        public void Fit(int extra)
        {
            int w = TextRenderer.MeasureText(Text, Font).Width + 24 + (string.IsNullOrEmpty(Glyph) ? 0 : GlyphWidth() + 8);
            Width = Math.Max(60, w + extra);
        }
    }

    public class ToggleSwitch : Control
    {
        private bool _checked;
        public event EventHandler CheckedChanged;
        private bool _hover;
        private float _pos;
        private float _target;
        private System.Windows.Forms.Timer _anim;

        public ToggleSwitch()
        {
            SetStyle(ControlStyles.AllPaintingInWmPaint | ControlStyles.UserPaint | ControlStyles.OptimizedDoubleBuffer | ControlStyles.SupportsTransparentBackColor | ControlStyles.ResizeRedraw, true);
            BackColor = Color.Transparent;
            Size = new Size(46, 24);
            Cursor = Cursors.Hand;
        }

        public bool Checked
        {
            get { return _checked; }
            set
            {
                if (_checked == value) return;
                _checked = value;
                AnimateTo(value ? 1f : 0f);
                Invalidate();
                EventHandler h = CheckedChanged;
                if (h != null) h(this, EventArgs.Empty);
            }
        }

        /// <summary>Задаёт состояние без плавной анимации (при загрузке данных).</summary>
        public void SetCheckedSilent(bool value)
        {
            _checked = value;
            _pos = value ? 1f : 0f;
            _target = _pos;
            Invalidate();
        }

        private void AnimateTo(float target)
        {
            _target = target;
            if (!IsHandleCreated)
            {
                _pos = target;
                return;
            }
            if (_anim == null)
            {
                _anim = new System.Windows.Forms.Timer();
                _anim.Interval = 15;
                _anim.Tick += delegate
                {
                    float diff = _target - _pos;
                    if (Math.Abs(diff) < 0.06f)
                    {
                        _pos = _target;
                        _anim.Stop();
                    }
                    else
                    {
                        _pos += diff * 0.35f;
                    }
                    Invalidate();
                };
            }
            _anim.Start();
        }

        protected override void OnMouseEnter(EventArgs e) { _hover = true; Invalidate(); base.OnMouseEnter(e); }
        protected override void OnMouseLeave(EventArgs e) { _hover = false; Invalidate(); base.OnMouseLeave(e); }
        protected override void OnClick(EventArgs e)
        {
            if (Enabled) Checked = !Checked;
            base.OnClick(e);
        }

        protected override void OnEnabledChanged(EventArgs e) { Invalidate(); base.OnEnabledChanged(e); }

        protected override void Dispose(bool disposing)
        {
            if (disposing && _anim != null) { _anim.Stop(); _anim.Dispose(); _anim = null; }
            base.Dispose(disposing);
        }

        protected override void OnPaint(PaintEventArgs e)
        {
            Graphics g = e.Graphics;
            g.SmoothingMode = SmoothingMode.AntiAlias;
            int h = Math.Min(Height - 2, 24);
            int w = Math.Min(Width - 2, 46);
            int top = (Height - h) / 2;
            Rectangle track = new Rectangle(1, top, w - 1, h - 1);
            int radius = track.Height / 2;

            if (!Enabled)
            {
                Draw.FillRound(g, track, radius, Theme.Surface);
                Draw.DrawRound(g, track, radius, Theme.Border, 1f);
            }
            else if (_pos > 0.02f)
            {
                Color on = Blend(Theme.SurfaceAlt, Theme.Accent, _pos);
                Draw.FillRound(g, track, radius, on);
                Draw.DrawRound(g, track, radius, Theme.WithAlpha(Color.White, 18), 1f);
            }
            else
            {
                Draw.FillRound(g, track, radius, _hover ? Theme.BorderStrong : Theme.SurfaceAlt);
                Draw.DrawRound(g, track, radius, _hover ? Theme.BorderStrong : Theme.Border, 1f);
            }

            int knob = track.Height - 6;
            int kx = track.X + 3 + (int)((track.Width - 6 - knob) * _pos);
            int ky = track.Y + 3;
            using (SolidBrush shadow = new SolidBrush(Color.FromArgb(45, 0, 0, 0)))
            {
                g.FillEllipse(shadow, kx, ky + 1, knob, knob);
            }
            using (SolidBrush b = new SolidBrush(Enabled ? Color.White : Color.FromArgb(210, 202, 194)))
            {
                g.FillEllipse(b, kx, ky, knob, knob);
            }
        }

        private static Color Blend(Color from, Color to, float t)
        {
            if (t < 0) t = 0;
            if (t > 1) t = 1;
            return Color.FromArgb(
                (int)(from.R + (to.R - from.R) * t),
                (int)(from.G + (to.G - from.G) * t),
                (int)(from.B + (to.B - from.B) * t));
        }
    }

    public class NavButton : Control
    {
        public string Glyph = "";
        public bool Selected;
        public string Badge = "";
        private bool _hover;

        public NavButton(string text, string glyph)
        {
            SetStyle(ControlStyles.AllPaintingInWmPaint | ControlStyles.UserPaint | ControlStyles.OptimizedDoubleBuffer | ControlStyles.SupportsTransparentBackColor | ControlStyles.ResizeRedraw, true);
            Text = text;
            Glyph = glyph;
            BackColor = Color.Transparent;
            Font = Theme.Base;
            Height = 42;
            Cursor = Cursors.Hand;
        }

        protected override void OnMouseEnter(EventArgs e) { _hover = true; Invalidate(); base.OnMouseEnter(e); }
        protected override void OnMouseLeave(EventArgs e) { _hover = false; Invalidate(); base.OnMouseLeave(e); }

        protected override void OnPaint(PaintEventArgs e)
        {
            Graphics g = e.Graphics;
            g.SmoothingMode = SmoothingMode.AntiAlias;
            if (Selected)
            {
                Draw.FillRound(g, new Rectangle(10, 2, Width - 20, Height - 4), 9, Theme.Selection);
                Draw.FillRound(g, new Rectangle(10, 11, 3, Height - 22), 2, Theme.Accent);
            }
            else if (_hover)
            {
                Draw.FillRound(g, new Rectangle(10, 2, Width - 20, Height - 4), 9, Theme.Hover);
            }

            Color textColor = Selected ? Theme.Text : (_hover ? Theme.Text : Theme.TextDim);
            int x = 24;
            if (!string.IsNullOrEmpty(Glyph))
            {
                TextRenderer.DrawText(g, Glyph, Theme.Icons, new Rectangle(x, 0, 22, Height), Selected ? Theme.AccentTop : Theme.TextMuted,
                    TextFormatFlags.Left | TextFormatFlags.VerticalCenter | TextFormatFlags.NoPrefix);
                x += 28;
            }
            TextRenderer.DrawText(g, Text, Selected ? Theme.Bold : Theme.Base, new Rectangle(x, 0, Width - x - 12, Height), textColor,
                TextFormatFlags.Left | TextFormatFlags.VerticalCenter | TextFormatFlags.NoPrefix);
        }
    }

    public class Pill : Control
    {
        public Color PillColor = Theme.Accent;
        public bool Filled = true;

        public Pill(string text, Color color)
        {
            SetStyle(ControlStyles.AllPaintingInWmPaint | ControlStyles.UserPaint | ControlStyles.OptimizedDoubleBuffer | ControlStyles.SupportsTransparentBackColor | ControlStyles.ResizeRedraw, true);
            Text = text;
            PillColor = color;
            BackColor = Color.Transparent;
            Font = Theme.Small;
            Height = 22;
            Width = 90;
        }

        public void Fit()
        {
            Width = Math.Max(40, TextRenderer.MeasureText(Text, Font).Width + 18);
        }

        protected override void OnPaint(PaintEventArgs e)
        {
            Graphics g = e.Graphics;
            g.SmoothingMode = SmoothingMode.AntiAlias;
            Rectangle r = new Rectangle(0, 0, Width - 1, Height - 1);
            Draw.FillRound(g, r, Height / 2, Theme.WithAlpha(PillColor, Filled ? 46 : 28));
            Draw.DrawRound(g, r, Height / 2, Theme.WithAlpha(PillColor, 110), 1f);
            TextRenderer.DrawText(g, Text, Font, ClientRectangle, PillColor, Draw.Center);
        }
    }

    public class LineProgress : Control
    {
        private int _value;
        public string Caption = "";

        public LineProgress()
        {
            SetStyle(ControlStyles.AllPaintingInWmPaint | ControlStyles.UserPaint | ControlStyles.OptimizedDoubleBuffer | ControlStyles.SupportsTransparentBackColor | ControlStyles.ResizeRedraw, true);
            BackColor = Color.Transparent;
            Height = 8;
        }

        public int Value
        {
            get { return _value; }
            set { _value = Math.Max(0, Math.Min(100, value)); Invalidate(); }
        }

        protected override void OnPaint(PaintEventArgs e)
        {
            Graphics g = e.Graphics;
            g.SmoothingMode = SmoothingMode.AntiAlias;
            Draw.FillRound(g, new Rectangle(0, 0, Width - 1, Height - 1), Height / 2, Theme.SurfaceAlt);
            int w = (int)((Width - 1) * _value / 100.0);
            if (w > 2) Draw.FillRound(g, new Rectangle(0, 0, w, Height - 1), Height / 2, Theme.Accent);
        }
    }

    public class StatusDot : Control
    {
        public Color DotColor = Theme.TextMuted;
        private bool _pulse;

        public StatusDot()
        {
            SetStyle(ControlStyles.AllPaintingInWmPaint | ControlStyles.UserPaint | ControlStyles.OptimizedDoubleBuffer | ControlStyles.SupportsTransparentBackColor | ControlStyles.ResizeRedraw, true);
            BackColor = Color.Transparent;
            Size = new Size(10, 10);
        }

        public void SetColor(Color c, bool pulse)
        {
            DotColor = c;
            _pulse = pulse;
            Invalidate();
        }

        protected override void OnPaint(PaintEventArgs e)
        {
            Graphics g = e.Graphics;
            g.SmoothingMode = SmoothingMode.AntiAlias;
            if (_pulse) Draw.FillRound(g, new Rectangle(0, 0, Width - 1, Height - 1), Width / 2, Theme.WithAlpha(DotColor, 60));
            int inset = _pulse ? 2 : 1;
            using (SolidBrush b = new SolidBrush(DotColor)) g.FillEllipse(b, inset, inset, Width - inset * 2 - 1, Height - inset * 2 - 1);
        }
    }

    /// <summary>Тёмное текстовое поле с рамкой.</summary>
    public class InputBox : Panel
    {
        public TextBox Box;
        public bool Multiline;

        public InputBox()
        {
            DoubleBuffered = true;
            BackColor = Theme.Surface;
            Height = 32;
            Width = 200;
            Padding = new Padding(8, 0, 8, 0);
            Box = new TextBox();
            Box.BorderStyle = BorderStyle.None;
            Box.BackColor = Theme.Surface;
            Box.ForeColor = Theme.Text;
            Box.Font = Theme.Base;
            Controls.Add(Box);
        }

        public bool UsePasswordChars;

        protected override void OnResize(EventArgs e)
        {
            base.OnResize(e);
            if (Box == null) return;
            Box.Location = new Point(8, (Height - Box.PreferredHeight) / 2);
            Box.Width = Width - 16;
        }

        protected override void OnPaint(PaintEventArgs e)
        {
            base.OnPaint(e);
            Rectangle r = new Rectangle(0, 0, Width - 1, Height - 1);
            Draw.DrawRound(e.Graphics, r, 7, Box.Focused ? Theme.AccentTop : Theme.Border, 1f);
        }

        public override string Text
        {
            get { return Box.Text; }
            set { Box.Text = value; }
        }
    }

    /// <summary>Тёмный журнал событий.</summary>
    public class LogView : RichTextBox
    {
        public LogView()
        {
            BorderStyle = BorderStyle.None;
            BackColor = Theme.Bg;
            ForeColor = Theme.TextDim;
            Font = Theme.Mono;
            ReadOnly = true;
            DetectUrls = false;
            WordWrap = true;
            ScrollBars = RichTextBoxScrollBars.Vertical;
            HideSelection = true;
            ShortcutsEnabled = true;
        }

        protected override void OnHandleCreated(EventArgs e)
        {
            base.OnHandleCreated(e);
            Native.ApplyDarkScrollbars(Handle);
        }

        public void Append(LogLevel level, string text)
        {
            Color color;
            switch (level)
            {
                case LogLevel.Ok: color = Theme.Success; break;
                case LogLevel.Warn: color = Theme.Warn; break;
                case LogLevel.Error: color = Theme.Danger; break;
                default: color = Theme.TextDim; break;
            }
            if (TextLength > 60000)
            {
                Clear();
                SelectionColor = Theme.TextMuted;
                AppendText("… журнал очищен …" + Environment.NewLine);
            }
            SelectionStart = TextLength;
            SelectionLength = 0;
            SelectionColor = color;
            AppendText(text + Environment.NewLine);
            SelectionStart = TextLength;
            ScrollToCaret();
        }
    }

    /// <summary>Контейнер с вертикальной раскладкой.</summary>
    public class VStack : Panel
    {
        public int Gap = 12;
        public int ContentHeight = 0;
        public event EventHandler LayoutDone;
        private readonly List<Control> _items = new List<Control>();

        public VStack()
        {
            DoubleBuffered = true;
            SetStyle(ControlStyles.OptimizedDoubleBuffer | ControlStyles.AllPaintingInWmPaint, true);
            BackColor = Theme.Bg;
        }

        public void Add(Control c)
        {
            _items.Add(c);
            Controls.Add(c);
            PerformLayout();
        }

        public void ClearItems()
        {
            foreach (Control c in _items.ToArray())
            {
                Controls.Remove(c);
                c.Dispose();
            }
            _items.Clear();
            PerformLayout();
        }

        protected override void OnLayout(LayoutEventArgs e)
        {
            base.OnLayout(e);
            int y = 0;
            int w = Math.Max(400, ClientSize.Width);
            foreach (Control c in _items)
            {
                if (!c.Visible) continue;
                c.Location = new Point(0, y);
                if (c.Width != w) c.Width = w;
                y += c.Height + Gap;
            }
            ContentHeight = y;
            EventHandler h = LayoutDone;
            if (h != null) h(this, EventArgs.Empty);
        }

        protected override void OnResize(EventArgs e)
        {
            base.OnResize(e);
            PerformLayout();
        }
    }

    /// <summary>Прокручиваемый контейнер для VStack с собственным тёмным ползунком.</summary>
    public class ScrollHost : Panel
    {
        public VStack Stack;
        private int _offset;
        private bool _dragging;
        private int _dragStartY;
        private int _dragStartOffset;
        private bool _hoverBar;

        private const int BarGap = 6;
        private const int BarWidth = 7;
        private const int ThumbMin = 36;

        public ScrollHost()
        {
            SetStyle(ControlStyles.OptimizedDoubleBuffer | ControlStyles.AllPaintingInWmPaint | ControlStyles.ResizeRedraw | ControlStyles.UserPaint, true);
            AutoScroll = false;
            BackColor = Theme.Bg;
            Stack = new VStack();
            Stack.Location = new Point(0, 0);
            Controls.Add(Stack);
            Stack.LayoutDone += delegate { SyncHeight(); };
        }

        // WS_EX_COMPOSITED — композитная отрисовка поддерева: убирает «разрыв» картинки при прокрутке
        protected override CreateParams CreateParams
        {
            get
            {
                CreateParams cp = base.CreateParams;
                cp.ExStyle |= 0x02000000;
                return cp;
            }
        }

        private int MaxOffset
        {
            get { return Math.Max(0, Stack.ContentHeight - ClientSize.Height); }
        }

        private int TrackHeight
        {
            get { return Math.Max(0, ClientSize.Height - 8); }
        }

        private int ThumbHeight
        {
            get
            {
                if (Stack.ContentHeight <= 0) return 0;
                int h = (int)((double)ClientSize.Height * ClientSize.Height / Stack.ContentHeight);
                return Math.Max(ThumbMin, Math.Min(TrackHeight, h));
            }
        }

        private int BarLeft
        {
            get { return ClientSize.Width - BarWidth - BarGap; }
        }

        protected override void OnResize(EventArgs e)
        {
            base.OnResize(e);
            SyncWidth();
            ApplyOffset();
            Invalidate();
        }

        private void SyncWidth()
        {
            int w = ClientSize.Width - BarWidth - BarGap - 2;
            if (w < 400) w = Math.Max(200, ClientSize.Width - 2);
            Stack.Width = w;
        }

        private void SyncHeight()
        {
            int h = Math.Max(10, Stack.ContentHeight);
            if (Stack.Height != h) Stack.Height = h;
            if (_offset > MaxOffset) _offset = MaxOffset;
            ApplyOffset();
            Invalidate();
        }

        private void ApplyOffset()
        {
            if (_offset > MaxOffset) _offset = MaxOffset;
            if (_offset < 0) _offset = 0;
            Stack.Location = new Point(0, -_offset);
        }

        /// <summary>Перейти к позиции прокрутки (px от начала).</summary>
        public void ScrollTo(int offset)
        {
            _offset = Math.Max(0, Math.Min(MaxOffset, offset));
            Log.Info(string.Format("Прокрутка: запрошено {0}, максимум {1}, content {2}, стек {3}, окно {4}",
                offset, MaxOffset, Stack.ContentHeight, Stack.Height, ClientSize.Height));
            ApplyOffset();
            Invalidate();
        }

        /// <summary>Прокрутка колесом (delta как в MouseEventArgs).</summary>
        public void Wheel(int delta)
        {
            if (MaxOffset <= 0) return;
            _offset -= delta / 3;
            if (_offset < 0) _offset = 0;
            if (_offset > MaxOffset) _offset = MaxOffset;
            ApplyOffset();
            Invalidate();
        }

        protected override void OnMouseWheel(MouseEventArgs e)
        {
            Wheel(e.Delta);
            base.OnMouseWheel(e);
        }

        protected override void OnMouseDown(MouseEventArgs e)
        {
            base.OnMouseDown(e);
            if (MaxOffset <= 0) return;
            if (e.X >= BarLeft - 4)
            {
                if (e.Y >= ThumbTop() && e.Y <= ThumbTop() + ThumbHeight)
                {
                    _dragging = true;
                    _dragStartY = e.Y;
                    _dragStartOffset = _offset;
                }
                else
                {
                    int target = (int)((e.Y - 4.0) / TrackHeight * MaxOffset);
                    _offset = target;
                    ApplyOffset();
                    Invalidate();
                }
            }
        }

        private int ThumbTop()
        {
            if (MaxOffset <= 0) return 4;
            int space = TrackHeight - ThumbHeight;
            return 4 + (int)((double)_offset / MaxOffset * space);
        }

        protected override void OnMouseMove(MouseEventArgs e)
        {
            base.OnMouseMove(e);
            bool hover = MaxOffset > 0 && e.X >= BarLeft - 4;
            if (hover != _hoverBar)
            {
                _hoverBar = hover;
                Invalidate();
            }
            if (_dragging)
            {
                int space = TrackHeight - ThumbHeight;
                if (space > 0)
                {
                    int delta = e.Y - _dragStartY;
                    _offset = _dragStartOffset + (int)((double)delta / space * MaxOffset);
                    ApplyOffset();
                    Invalidate();
                }
            }
        }

        protected override void OnMouseUp(MouseEventArgs e)
        {
            base.OnMouseUp(e);
            _dragging = false;
        }

        protected override void OnMouseLeave(EventArgs e)
        {
            base.OnMouseLeave(e);
            _hoverBar = false;
            Invalidate();
        }

        protected override void OnPaint(PaintEventArgs e)
        {
            base.OnPaint(e);
            if (MaxOffset <= 0) return;
            Graphics g = e.Graphics;
            g.SmoothingMode = SmoothingMode.AntiAlias;
            int top = ThumbTop();
            int h = ThumbHeight;
            using (SolidBrush b = new SolidBrush(_hoverBar || _dragging ? Color.FromArgb(110, 255, 255, 255) : Color.FromArgb(52, 255, 255, 255)))
            {
                using (GraphicsPath path = Draw.RoundRect(new Rectangle(BarLeft, top, BarWidth, h), BarWidth / 2))
                {
                    g.FillPath(b, path);
                }
            }
        }
    }

    /// <summary>Список стратегий с собственной отрисовкой и тёмным ползунком.</summary>
    public class StrategyList : Control
    {
        public List<StrategyDef> Items2 = new List<StrategyDef>();
        public string ActiveName = "";
        public event EventHandler SelectedIndexChanged;
        public event EventHandler ItemActivated;

        private int _selected = -1;
        private int _scroll;
        private int _hover = -1;
        private bool _dragBar;
        private const int RowH = 56;
        private const int BarW = 6;

        public StrategyList()
        {
            SetStyle(ControlStyles.AllPaintingInWmPaint | ControlStyles.UserPaint | ControlStyles.OptimizedDoubleBuffer | ControlStyles.ResizeRedraw, true);
            BackColor = Theme.Surface;
            Font = Theme.Base;
            Cursor = Cursors.Hand;
        }

        public int SelectedIndex
        {
            get { return _selected; }
            set
            {
                int v = (value < 0 || value >= Items2.Count) ? -1 : value;
                if (v == _selected) return;
                _selected = v;
                EnsureVisible();
                Invalidate();
                EventHandler h = SelectedIndexChanged;
                if (h != null) h(this, EventArgs.Empty);
            }
        }

        public StrategyDef SelectedItem
        {
            get { return (_selected >= 0 && _selected < Items2.Count) ? Items2[_selected] : null; }
        }

        public void SetItems(List<StrategyDef> items)
        {
            Items2 = items;
            _scroll = 0;
            if (_selected >= items.Count) _selected = -1;
            Invalidate();
        }

        private int ContentHeight { get { return Items2.Count * RowH + 8; } }
        private int MaxScroll { get { return Math.Max(0, ContentHeight - ClientSize.Height); } }

        private void EnsureVisible()
        {
            if (_selected < 0) return;
            int top = _selected * RowH;
            int bottom = top + RowH;
            if (top < _scroll) _scroll = top;
            if (bottom > _scroll + ClientSize.Height) _scroll = bottom - ClientSize.Height;
            if (_scroll < 0) _scroll = 0;
            if (_scroll > MaxScroll) _scroll = MaxScroll;
        }

        protected override void OnMouseWheel(MouseEventArgs e)
        {
            if (MaxScroll > 0)
            {
                _scroll -= e.Delta / 2;
                if (_scroll < 0) _scroll = 0;
                if (_scroll > MaxScroll) _scroll = MaxScroll;
                Invalidate();
            }
            base.OnMouseWheel(e);
        }

        protected override void OnMouseMove(MouseEventArgs e)
        {
            base.OnMouseMove(e);
            int idx = IndexAt(e.Y);
            if (idx != _hover) { _hover = idx; Invalidate(); }
            if (_dragBar) ScrollToThumb(e.Y);
        }

        protected override void OnMouseLeave(EventArgs e)
        {
            base.OnMouseLeave(e);
            _hover = -1;
            Invalidate();
        }

        private int IndexAt(int y)
        {
            if (MaxScroll > 0 && y > 0)
            {
                int row = (y + _scroll - 4) / RowH;
                if (row >= 0 && row < Items2.Count) return row;
            }
            int r = (y + _scroll - 4) / RowH;
            return (r >= 0 && r < Items2.Count) ? r : -1;
        }

        protected override void OnMouseDown(MouseEventArgs e)
        {
            base.OnMouseDown(e);
            if (MaxScroll > 0 && e.X >= ClientSize.Width - BarW - 6)
            {
                _dragBar = true;
                ScrollToThumb(e.Y);
                return;
            }
            int idx = IndexAt(e.Y);
            if (idx >= 0) SelectedIndex = idx;
        }

        protected override void OnMouseUp(MouseEventArgs e)
        {
            base.OnMouseUp(e);
            _dragBar = false;
        }

        protected override void OnMouseDoubleClick(MouseEventArgs e)
        {
            base.OnMouseDoubleClick(e);
            int idx = IndexAt(e.Y);
            if (idx >= 0)
            {
                SelectedIndex = idx;
                EventHandler h = ItemActivated;
                if (h != null) h(this, EventArgs.Empty);
            }
        }

        private void ScrollToThumb(int y)
        {
            if (MaxScroll <= 0) return;
            int track = ClientSize.Height - 8 - ThumbH;
            if (track <= 0) return;
            double ratio = (double)(y - 4 - ThumbH / 2) / track;
            if (ratio < 0) ratio = 0;
            if (ratio > 1) ratio = 1;
            _scroll = (int)(ratio * MaxScroll);
            Invalidate();
        }

        private int ThumbH
        {
            get
            {
                if (ContentHeight <= 0) return 0;
                int h = (int)((double)ClientSize.Height * ClientSize.Height / ContentHeight);
                return Math.Max(32, Math.Min(ClientSize.Height - 8, h));
            }
        }

        private int ThumbTop
        {
            get
            {
                int track = ClientSize.Height - 8 - ThumbH;
                if (track <= 0 || MaxScroll <= 0) return 4;
                return 4 + (int)((double)_scroll / MaxScroll * track);
            }
        }

        protected override void OnPaint(PaintEventArgs e)
        {
            Graphics g = e.Graphics;
            g.SmoothingMode = SmoothingMode.AntiAlias;
            using (SolidBrush b = new SolidBrush(Theme.Surface)) g.FillRectangle(b, ClientRectangle);

            int first = Math.Max(0, (_scroll - RowH) / RowH);
            int last = Math.Min(Items2.Count - 1, (_scroll + ClientSize.Height) / RowH + 1);
            for (int i = first; i <= last; i++)
            {
                StrategyDef s = Items2[i];
                int top = 4 + i * RowH - _scroll;
                Rectangle r = new Rectangle(4, top, ClientSize.Width - 8 - (MaxScroll > 0 ? BarW + 4 : 0), RowH - 4);
                bool selected = i == _selected;
                bool active = string.Equals(s.Name, ActiveName, StringComparison.OrdinalIgnoreCase);
                Color fill = selected ? Theme.Selection : (i == _hover ? Theme.Hover : Theme.Surface);
                Draw.FillRound(g, r, 8, fill);
                if (active) Draw.FillRound(g, new Rectangle(r.X, r.Y + 10, 3, r.Height - 20), 2, Theme.Success);

                int x = r.X + 14;
                int badgeW = 84;
                TextRenderer.DrawText(g, s.Name, Theme.Bold, new Rectangle(x, r.Y + 9, r.Width - badgeW - 30, 18),
                    selected || active ? Theme.Text : Theme.Text, TextFormatFlags.Left | TextFormatFlags.NoPrefix | TextFormatFlags.EndEllipsis);
                TextRenderer.DrawText(g, s.Description, Theme.Small, new Rectangle(x, r.Y + 28, r.Width - badgeW - 30, 16),
                    Theme.TextMuted, TextFormatFlags.Left | TextFormatFlags.NoPrefix | TextFormatFlags.EndEllipsis);

                string badge = s.ShortName;
                int bw = Math.Min(badgeW, TextRenderer.MeasureText(badge, Theme.Small).Width + 18);
                Rectangle badgeRect = new Rectangle(r.Right - bw - 12, r.Y + (r.Height - 20) / 2, bw, 20);
                Color badgeColor = BadgeColor(s.Kind);
                Draw.FillRound(g, badgeRect, 10, Theme.WithAlpha(badgeColor, 40));
                TextRenderer.DrawText(g, badge, Theme.Small, badgeRect, badgeColor, Draw.Center);

                if (active)
                {
                    TextRenderer.DrawText(g, "активна", Theme.Small, new Rectangle(badgeRect.X - 68, badgeRect.Y + 2, 60, 16),
                        Theme.Success, TextFormatFlags.Right | TextFormatFlags.VerticalCenter | TextFormatFlags.NoPrefix);
                }
            }

            if (MaxScroll > 0)
            {
                using (SolidBrush b = new SolidBrush(Color.FromArgb(_hover >= 0 ? 110 : 58, 255, 214, 170)))
                using (GraphicsPath path = Draw.RoundRect(new Rectangle(ClientSize.Width - BarW - 4, ThumbTop, BarW, ThumbH), BarW / 2))
                {
                    g.FillPath(b, path);
                }
            }
        }

        public static Color BadgeColor(string kind)
        {
            if (kind == "base") return Theme.Success;
            if (kind == "alt") return Theme.Accent;
            if (kind == "exp") return Theme.Danger;
            if (kind == "simple") return Theme.Warn;
            if (kind == "faketls") return Color.FromArgb(214, 178, 130);
            return Theme.TextDim;
        }
    }

    public static class UIHelper
    {
        public static Label Label(string text, Font font, Color color)
        {
            Label l = new Label();
            l.Text = text;
            l.Font = font;
            l.ForeColor = color;
            l.AutoSize = true;
            l.BackColor = Color.Transparent;
            return l;
        }

        public static Label Wrap(string text, Font font, Color color, int width)
        {
            Label l = new Label();
            l.Text = text;
            l.Font = font;
            l.ForeColor = color;
            l.AutoSize = false;
            l.BackColor = Color.Transparent;
            l.Width = width;
            l.Height = TextRenderer.MeasureText(text, font, new Size(width, 1000), TextFormatFlags.WordBreak).Height + 2;
            return l;
        }

        public static void Place(Control parent, Control child, int x, int y)
        {
            child.Location = new Point(x, y);
            parent.Controls.Add(child);
        }
    }
}
