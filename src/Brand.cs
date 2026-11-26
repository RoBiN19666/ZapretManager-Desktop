using System;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Drawing.Imaging;
using System.IO;
using System.Reflection;

namespace ZapretManager
{
    /// <summary>Встроенные изображения бренда (иконка и логотип вшиты в exe).</summary>
    public static class Brand
    {
        private static bool _loaded;
        private static Bitmap _icon;
        private static Bitmap _logo;

        public static Bitmap Icon
        {
            get { EnsureLoaded(); return _icon; }
        }

        public static Bitmap Logo
        {
            get { EnsureLoaded(); return _logo; }
        }

        private static void EnsureLoaded()
        {
            if (_loaded) return;
            _loaded = true;
            _icon = Load("app-icon.png");
            _logo = Load("logo-wide.png");
        }

        private static Bitmap Load(string name)
        {
            try
            {
                using (Stream s = Assembly.GetExecutingAssembly().GetManifestResourceStream(name))
                {
                    if (s == null) return null;
                    using (Image img = Image.FromStream(s))
                    {
                        return new Bitmap(img);
                    }
                }
            }
            catch (Exception ex)
            {
                Log.Warn("Не удалось загрузить изображение " + name + ": " + ex.Message);
                return null;
            }
        }

        /// <summary>Рисует квадратную иконку бренда в указанных границах.</summary>
        public static void DrawIcon(Graphics g, Rectangle bounds)
        {
            Bitmap icon = Icon;
            if (icon == null) return;
            InterpolationMode old = g.InterpolationMode;
            g.InterpolationMode = InterpolationMode.HighQualityBicubic;
            g.PixelOffsetMode = PixelOffsetMode.HighQuality;
            try
            {
                g.DrawImage(icon, bounds);
            }
            finally
            {
                g.InterpolationMode = old;
            }
        }

        /// <summary>Рисует широкий логотип в указанных границах (с сохранением пропорций).</summary>
        public static bool DrawLogo(Graphics g, Rectangle bounds, ContentAlignment align)
        {
            Bitmap logo = Logo;
            if (logo == null) return false;
            InterpolationMode old = g.InterpolationMode;
            g.InterpolationMode = InterpolationMode.HighQualityBicubic;
            g.PixelOffsetMode = PixelOffsetMode.HighQuality;
            try
            {
                float scale = Math.Min((float)bounds.Width / logo.Width, (float)bounds.Height / logo.Height);
                int w = Math.Max(1, (int)(logo.Width * scale));
                int h = Math.Max(1, (int)(logo.Height * scale));
                int x = bounds.X + (bounds.Width - w) / 2;
                int y = bounds.Y + (bounds.Height - h) / 2;
                if (align == ContentAlignment.MiddleLeft) x = bounds.X;
                g.DrawImage(logo, new Rectangle(x, y, w, h));
                return true;
            }
            finally
            {
                g.InterpolationMode = old;
            }
        }
    }
}
