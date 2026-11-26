using System;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Windows.Forms;

namespace ZapretManager
{
    public sealed class DialogForm : Form
    {
        private bool _ok;
        private int _choice;

        public DialogForm(string title, string message, string okText, bool danger, bool withCancel)
            : this(title, message, okText, danger, withCancel, null)
        {
        }

        /// <summary>Диалог с третьей кнопкой (например, «Не сохранять»); Choice: 1 — OK, 2 — третья, 0 — отмена.</summary>
        public DialogForm(string title, string message, string okText, bool danger, bool withCancel, string extraText)
        {
            FormBorderStyle = FormBorderStyle.FixedDialog;
            StartPosition = FormStartPosition.CenterParent;
            MaximizeBox = false;
            MinimizeBox = false;
            ShowInTaskbar = false;
            BackColor = Theme.Bg;
            ForeColor = Theme.Text;
            Font = Theme.Base;
            Text = title;
            AutoScaleMode = AutoScaleMode.None;

            int width = extraText != null ? 640 : 520;
            int textWidth = width - 56;
            Font msgFont = Theme.Base;
            Size textSize = TextRenderer.MeasureText(message, msgFont, new Size(textWidth, 2000), TextFormatFlags.WordBreak);
            int height = 86 + textSize.Height + 60;
            ClientSize = new Size(width, height);

            Label lblTitle = UI2.Label(title, Theme.H2, Theme.Text, 28, 22);
            Controls.Add(lblTitle);

            Label lblMsg = new Label();
            lblMsg.Text = message;
            lblMsg.Font = msgFont;
            lblMsg.ForeColor = Theme.TextDim;
            lblMsg.BackColor = Color.Transparent;
            lblMsg.AutoSize = false;
            lblMsg.Bounds = new Rectangle(28, 58, textWidth, textSize.Height + 8);
            Controls.Add(lblMsg);

            RoundButton ok = new RoundButton();
            ok.Text = okText;
            ok.Kind = ButtonKind.Primary;
            ok.Width = Math.Max(110, TextRenderer.MeasureText(okText, Theme.Bold).Width + 40);
            ok.Height = 34;
            ok.Location = new Point(ClientSize.Width - 28 - ok.Width, ClientSize.Height - 54);
            ok.Click += delegate { _ok = true; _choice = 1; DialogResult = DialogResult.OK; Close(); };
            Controls.Add(ok);

            if (withCancel)
            {
                RoundButton cancel = new RoundButton();
                cancel.Text = "Отмена";
                cancel.Kind = ButtonKind.Secondary;
                cancel.Width = 100;
                cancel.Height = 34;
                cancel.Location = new Point(ok.Left - 112, ClientSize.Height - 54);
                cancel.Click += delegate { _choice = 0; DialogResult = DialogResult.Cancel; Close(); };
                Controls.Add(cancel);
                CancelButton = cancel;

                if (extraText != null)
                {
                    RoundButton extra = new RoundButton();
                    extra.Text = extraText;
                    extra.Kind = ButtonKind.Ghost;
                    extra.Width = Math.Max(120, TextRenderer.MeasureText(extraText, Theme.Bold).Width + 36);
                    extra.Height = 34;
                    extra.Location = new Point(cancel.Left - 12 - extra.Width, ClientSize.Height - 54);
                    extra.Click += delegate { _choice = 2; DialogResult = DialogResult.No; Close(); };
                    Controls.Add(extra);
                }
            }
            AcceptButton = ok;
            if (danger) ok.Kind = ButtonKind.Danger;
            ok.Invalidate();

            Paint += delegate(object s, PaintEventArgs e)
            {
                e.Graphics.SmoothingMode = SmoothingMode.AntiAlias;
                Draw.DrawRound(e.Graphics, new Rectangle(0, 0, ClientSize.Width - 1, ClientSize.Height - 1), 12, Theme.Border, 1f);
            };
        }

        public bool Confirmed { get { return _ok; } }
        public int Choice { get { return _choice; } }

        protected override void OnHandleCreated(EventArgs e)
        {
            base.OnHandleCreated(e);
            Native.EnableDarkTitleBar(Handle);
        }
    }

    public static class Dlg
    {
        /// <summary>Возвращает 1 — сохранить, 2 — не сохранять, 0 — отмена.</summary>
        public static int SaveChanges(IWin32Window owner, string title, string message)
        {
            using (DialogForm f = new DialogForm(title, message, "Сохранить", false, true, "Не сохранять"))
            {
                f.ShowDialog(owner);
                return f.Choice;
            }
        }

        public static bool Confirm(IWin32Window owner, string title, string message, string okText)
        {
            using (DialogForm f = new DialogForm(title, message, okText, false, true))
            {
                f.ShowDialog(owner);
                return f.DialogResult == DialogResult.OK;
            }
        }

        public static bool ConfirmDanger(IWin32Window owner, string title, string message, string okText)
        {
            using (DialogForm f = new DialogForm(title, message, okText, true, true))
            {
                f.ShowDialog(owner);
                return f.DialogResult == DialogResult.OK;
            }
        }

        public static void Info(IWin32Window owner, string title, string message)
        {
            using (DialogForm f = new DialogForm(title, message, "Понятно", false, false))
            {
                f.ShowDialog(owner);
            }
        }

        public static bool Error(IWin32Window owner, string message)
        {
            return Confirm(owner, "Ошибка", message, "Закрыть");
        }
    }

    public static class UI2
    {
        public static Label Label(string text, Font font, Color color, int x, int y)
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
    }

    /// <summary>ComboBox в тёмной теме.</summary>
    public class DarkCombo : ComboBox
    {
        public DarkCombo()
        {
            DrawMode = DrawMode.OwnerDrawFixed;
            DropDownStyle = ComboBoxStyle.DropDownList;
            FlatStyle = FlatStyle.Flat;
            BackColor = Theme.SurfaceAlt;
            ForeColor = Theme.Text;
            Font = Theme.Base;
            ItemHeight = 22;
            Height = 30;
        }

        protected override void OnDrawItem(DrawItemEventArgs e)
        {
            if (e.Index < 0)
            {
                base.OnDrawItem(e);
                return;
            }
            Graphics g = e.Graphics;
            bool selected = (e.State & DrawItemState.Selected) == DrawItemState.Selected;
            using (SolidBrush b = new SolidBrush(selected ? Theme.Hover : Theme.SurfaceAlt))
            {
                g.FillRectangle(b, e.Bounds);
            }
            TextRenderer.DrawText(g, Convert.ToString(Items[e.Index]), Font, new Rectangle(e.Bounds.X + 6, e.Bounds.Y, e.Bounds.Width - 12, e.Bounds.Height),
                selected ? Theme.Text : Theme.TextDim, TextFormatFlags.Left | TextFormatFlags.VerticalCenter | TextFormatFlags.NoPrefix | TextFormatFlags.EndEllipsis);
        }

        protected override void OnGotFocus(EventArgs e)
        {
            base.OnGotFocus(e);
            Invalidate();
        }
    }

    /// <summary>Тёмный выпадающий список (замена ComboBox).</summary>
    public class DarkSelect : Control
    {
        public System.Collections.Generic.List<string> Items = new System.Collections.Generic.List<string>();
        public event EventHandler SelectedIndexChanged;
        private int _selected = -1;
        private bool _hover;

        public DarkSelect()
        {
            SetStyle(ControlStyles.AllPaintingInWmPaint | ControlStyles.UserPaint | ControlStyles.OptimizedDoubleBuffer | ControlStyles.SupportsTransparentBackColor | ControlStyles.ResizeRedraw | ControlStyles.Selectable, true);
            BackColor = Color.Transparent;
            Font = Theme.Base;
            Height = 30;
            Cursor = Cursors.Hand;
            TabStop = true;
        }

        public int SelectedIndex
        {
            get { return _selected; }
            set
            {
                if (value < -1 || value >= Items.Count) value = -1;
                if (value == _selected) return;
                _selected = value;
                Invalidate();
                EventHandler h = SelectedIndexChanged;
                if (h != null) h(this, EventArgs.Empty);
            }
        }

        public string SelectedItem
        {
            get { return (_selected >= 0 && _selected < Items.Count) ? Items[_selected] : ""; }
        }

        public void SetItems(System.Collections.Generic.List<string> items, string select)
        {
            Items = new System.Collections.Generic.List<string>(items);
            _selected = -1;
            for (int i = 0; i < Items.Count; i++)
            {
                if (string.Equals(Items[i], select, StringComparison.OrdinalIgnoreCase)) { _selected = i; break; }
            }
            if (_selected < 0 && Items.Count > 0) _selected = 0;
            Invalidate();
        }

        protected override void OnMouseEnter(EventArgs e) { _hover = true; Invalidate(); base.OnMouseEnter(e); }
        protected override void OnMouseLeave(EventArgs e) { _hover = false; Invalidate(); base.OnMouseLeave(e); }

        protected override void OnMouseDown(MouseEventArgs e)
        {
            base.OnMouseDown(e);
            OpenPopup();
        }

        protected override void OnGotFocus(EventArgs e) { Invalidate(); base.OnGotFocus(e); }
        protected override void OnLostFocus(EventArgs e) { Invalidate(); base.OnLostFocus(e); }

        protected override void OnKeyDown(KeyEventArgs e)
        {
            if (e.KeyCode == Keys.Enter || e.KeyCode == Keys.Space || e.KeyCode == Keys.Down)
            {
                OpenPopup();
                e.Handled = true;
                e.SuppressKeyPress = true;
            }
            base.OnKeyDown(e);
        }

        private void OpenPopup()
        {
            if (Items.Count == 0) return;
            Form owner = FindForm();
            Point screen = PointToScreen(new Point(0, Height));
            using (SelectPopup popup = new SelectPopup(Items, _selected, new Size(Width, 320), screen))
            {
                popup.ShowDialog(owner);
                if (popup.Result >= 0 && popup.Result < Items.Count)
                {
                    SelectedIndex = popup.Result;
                }
            }
        }

        protected override void OnPaint(PaintEventArgs e)
        {
            Graphics g = e.Graphics;
            g.SmoothingMode = SmoothingMode.AntiAlias;
            Rectangle r = new Rectangle(0, 0, Width - 1, Height - 1);
            Draw.FillRound(g, r, 7, Theme.SurfaceAlt);
            Draw.DrawRound(g, r, 7, _hover || Focused ? Theme.BorderStrong : Theme.Border, 1f);
            TextRenderer.DrawText(g, SelectedItem, Font, new Rectangle(10, 0, Width - 34, Height), Theme.Text,
                TextFormatFlags.Left | TextFormatFlags.VerticalCenter | TextFormatFlags.NoPrefix | TextFormatFlags.EndEllipsis);
            Point[] chevron = new Point[] {
                new Point(Width - 20, Height / 2 - 2),
                new Point(Width - 12, Height / 2 - 2),
                new Point(Width - 16, Height / 2 + 3)
            };
            using (SolidBrush b = new SolidBrush(Theme.TextDim)) g.FillPolygon(b, chevron);
        }
    }

    /// <summary>Всплывающий список для DarkSelect.</summary>
    public sealed class SelectPopup : Form
    {
        public int Result = -1;
        private readonly System.Collections.Generic.List<string> _items;
        private readonly int _selected;
        private readonly int _maxHeight;
        private int _offset;
        private int _hover = -1;
        private static int RowH { get { return Math.Max(28, (int)Math.Round(28 * Theme.S)); } }

        public SelectPopup(System.Collections.Generic.List<string> items, int selected, Size size, Point screenLocation)
        {
            _items = items;
            _selected = selected;
            _maxHeight = size.Height;
            FormBorderStyle = FormBorderStyle.None;
            ShowInTaskbar = false;
            StartPosition = FormStartPosition.Manual;
            BackColor = Theme.Surface;
            DoubleBuffered = true;
            SetStyle(ControlStyles.OptimizedDoubleBuffer | ControlStyles.AllPaintingInWmPaint | ControlStyles.UserPaint, true);
            Font = Theme.Base;
            int height = Math.Min(_maxHeight, _items.Count * RowH + 8);
            ClientSize = new Size(Math.Max(160, size.Width), height);

            Screen wa = Screen.FromPoint(screenLocation);
            int x = screenLocation.X;
            int y = screenLocation.Y;
            if (y + height > wa.WorkingArea.Bottom) y = screenLocation.Y - Height - size.Height - 4;
            if (y < wa.WorkingArea.Top) y = wa.WorkingArea.Top;
            if (x + Width > wa.WorkingArea.Right) x = wa.WorkingArea.Right - Width - 4;
            Location = new Point(x, y);

            if (_selected >= 0)
            {
                _hover = _selected;
                _offset = _selected * RowH - ClientSize.Height / 2 + RowH;
                int maxOffset = Math.Max(0, _items.Count * RowH + 8 - ClientSize.Height);
                if (_offset < 0) _offset = 0;
                if (_offset > maxOffset) _offset = maxOffset;
            }

            Deactivate += delegate { Close(); };
            MouseWheel += delegate(object s, MouseEventArgs e) { ScrollBy(-e.Delta / 2); };
            MouseMove += delegate(object s, MouseEventArgs e)
            {
                int idx = IndexAt(e.Y);
                if (idx != _hover) { _hover = idx; Invalidate(); }
            };
            MouseLeave += delegate { _hover = -1; Invalidate(); };
            MouseDown += delegate(object s, MouseEventArgs e)
            {
                int idx = IndexAt(e.Y);
                if (idx >= 0) { Result = idx; Close(); }
                else Close();
            };
            KeyDown += delegate(object s, KeyEventArgs e)
            {
                if (e.KeyCode == Keys.Escape) Close();
                else if (e.KeyCode == Keys.Down)
                {
                    _hover = _hover < 0 ? Math.Max(0, _selected) : Math.Min(_items.Count - 1, _hover + 1);
                    EnsureVisible(_hover);
                    Invalidate();
                }
                else if (e.KeyCode == Keys.Up)
                {
                    _hover = _hover < 0 ? Math.Max(0, _selected) : Math.Max(0, _hover - 1);
                    EnsureVisible(_hover);
                    Invalidate();
                }
                else if (e.KeyCode == Keys.Enter)
                {
                    int idx = _hover >= 0 ? _hover : _selected;
                    if (idx >= 0 && idx < _items.Count) { Result = idx; Close(); }
                }
            };
        }

        private void ScrollBy(int delta)
        {
            int maxOffset = Math.Max(0, _items.Count * RowH + 8 - ClientSize.Height);
            _offset += delta;
            if (_offset < 0) _offset = 0;
            if (_offset > maxOffset) _offset = maxOffset;
            Invalidate();
        }

        private void EnsureVisible(int index)
        {
            if (index < 0) return;
            int top = index * RowH + 4;
            if (top < _offset) _offset = top;
            else if (top + RowH > _offset + ClientSize.Height - 4) _offset = top + RowH - ClientSize.Height + 4;
            int maxOffset = Math.Max(0, _items.Count * RowH + 8 - ClientSize.Height);
            if (_offset < 0) _offset = 0;
            if (_offset > maxOffset) _offset = maxOffset;
        }

        private int IndexAt(int y)
        {
            int idx = (y + _offset - 4) / RowH;
            return (idx >= 0 && idx < _items.Count) ? idx : -1;
        }

        protected override void OnPaint(PaintEventArgs e)
        {
            Graphics g = e.Graphics;
            g.SmoothingMode = SmoothingMode.AntiAlias;
            using (SolidBrush b = new SolidBrush(Theme.Surface)) g.FillRectangle(b, ClientRectangle);

            for (int i = 0; i < _items.Count; i++)
            {
                int top = 4 + i * RowH - _offset;
                if (top < -RowH || top > ClientSize.Height) continue;
                bool isSel = i == _selected;
                if (i == _hover) Draw.FillRound(g, new Rectangle(4, top, ClientSize.Width - 8, RowH - 2), 6, Theme.Hover);
                else if (isSel) Draw.FillRound(g, new Rectangle(4, top, ClientSize.Width - 8, RowH - 2), 6, Theme.Selection);
                TextRenderer.DrawText(g, _items[i], Font, new Rectangle(12, top, ClientSize.Width - 24, RowH - 2),
                    isSel ? Theme.Accent : Theme.Text, TextFormatFlags.Left | TextFormatFlags.VerticalCenter | TextFormatFlags.NoPrefix | TextFormatFlags.EndEllipsis);
            }

            Draw.DrawRound(g, new Rectangle(0, 0, ClientSize.Width - 1, ClientSize.Height - 1), 8, Theme.BorderStrong, 1f);
        }

        protected override void OnHandleCreated(EventArgs e)
        {
            base.OnHandleCreated(e);
            TopMost = true;
        }
    }

    /// <summary>Многострочное поле ввода в тёмной теме.</summary>
    public class DarkEditor : RichTextBox
    {
        public DarkEditor()
        {
            BorderStyle = BorderStyle.None;
            BackColor = Theme.Editor;
            ForeColor = Theme.Text;
            Font = Theme.Mono;
            DetectUrls = false;
            WordWrap = false;
            ScrollBars = RichTextBoxScrollBars.Both;
            AcceptsTab = true;
        }

        /// <summary>Перенос по словам без горизонтального ползунка.</summary>
        public void SetWrapped()
        {
            WordWrap = true;
            ScrollBars = RichTextBoxScrollBars.Vertical;
        }

        public string Value
        {
            get { return Text; }
            set { Text = value; }
        }

        protected override void OnHandleCreated(EventArgs e)
        {
            base.OnHandleCreated(e);
            Native.ApplyDarkScrollbars(Handle);
        }
    }

    /// <summary>Список в тёмной теме.</summary>
    public class DarkList : ListBox
    {
        public DarkList()
        {
            SetStyle(ControlStyles.OptimizedDoubleBuffer | ControlStyles.AllPaintingInWmPaint, true);
            DoubleBuffered = true;
            DrawMode = DrawMode.OwnerDrawFixed;
            ItemHeight = 36;
            BorderStyle = BorderStyle.None;
            BackColor = Theme.Surface;
            ForeColor = Theme.Text;
            Font = Theme.Base;
            IntegralHeight = false;
        }

        public string[] Subtitles = new string[0];

        protected override void OnHandleCreated(EventArgs e)
        {
            base.OnHandleCreated(e);
            Native.ApplyDarkScrollbars(Handle);
        }

        protected override void OnDrawItem(DrawItemEventArgs e)
        {
            if (e.Index < 0 || e.Index >= Items.Count) return;
            Graphics g = e.Graphics;
            g.SmoothingMode = SmoothingMode.AntiAlias;
            bool selected = (e.State & DrawItemState.Selected) == DrawItemState.Selected;
            Rectangle r = new Rectangle(2, e.Bounds.Top + 1, e.Bounds.Width - 4, e.Bounds.Height - 2);
            using (SolidBrush back = new SolidBrush(BackColor)) g.FillRectangle(back, e.Bounds);
            if (selected) Draw.FillRound(g, r, 7, Theme.Selection);
            else if (e.Index == _hot) Draw.FillRound(g, r, 7, Theme.Hover);
            string sub = e.Index < Subtitles.Length ? Subtitles[e.Index] : "";
            TextRenderer.DrawText(g, Convert.ToString(Items[e.Index]), Theme.Bold, new Rectangle(r.X + 10, r.Y + 5, r.Width - 20, 16),
                selected ? Theme.Text : Theme.Text, TextFormatFlags.Left | TextFormatFlags.NoPrefix | TextFormatFlags.EndEllipsis);
            TextRenderer.DrawText(g, sub, Theme.Small, new Rectangle(r.X + 10, r.Y + 20, r.Width - 20, 14),
                Theme.TextMuted, TextFormatFlags.Left | TextFormatFlags.NoPrefix | TextFormatFlags.EndEllipsis);
        }

        private int _hot = -1;

        protected override void OnMouseMove(MouseEventArgs e)
        {
            base.OnMouseMove(e);
            int index = IndexFromPoint(e.Location);
            if (index == _hot) return;
            int old = _hot;
            _hot = index;
            if (old >= 0) InvalidateRow(old);
            if (_hot >= 0) InvalidateRow(_hot);
        }

        protected override void OnMouseLeave(EventArgs e)
        {
            base.OnMouseLeave(e);
            if (_hot >= 0) { InvalidateRow(_hot); _hot = -1; }
        }

        private void InvalidateRow(int index)
        {
            if (index < 0 || index >= Items.Count) return;
            try { Invalidate(GetItemRectangle(index)); }
            catch { Invalidate(); }
        }
    }

    /// <summary>Показ системных диалогов в тёмном оформлении.</summary>
    public static class DlgHost
    {
        public static void Message(IWin32Window owner, string text, string caption, MessageBoxIcon icon)
        {
            using (DarkDialogWatcher watcher = new DarkDialogWatcher())
            {
                MessageBox.Show(owner, text, caption, MessageBoxButtons.OK, icon);
            }
        }

        public static DialogResult Show(CommonDialog dialog, IWin32Window owner)
        {
            using (DarkDialogWatcher watcher = new DarkDialogWatcher())
            {
                try { return dialog.ShowDialog(owner); }
                finally { dialog.Dispose(); }
            }
        }
    }

    /// <summary>Небольшой кликабельный значок-ссылка (Boosty, DonationAlerts).</summary>'
    public sealed class LinkBadge : Control
    {
        public string Url = "";
        private Color _dot = Theme.Accent;
        private bool _hover;
        public EventHandler Clicked;

        public LinkBadge(string text, string url, Color dot)
        {
            SetStyle(ControlStyles.AllPaintingInWmPaint | ControlStyles.UserPaint | ControlStyles.OptimizedDoubleBuffer | ControlStyles.SupportsTransparentBackColor | ControlStyles.ResizeRedraw, true);
            Text = text;
            Url = url;
            _dot = dot;
            Font = Theme.Base;
            Height = 28;
            Width = TextRenderer.MeasureText(text, Font).Width + 46;
            Cursor = Cursors.Hand;
            BackColor = Color.Transparent;
        }

        protected override void OnMouseEnter(EventArgs e) { _hover = true; Invalidate(); base.OnMouseEnter(e); }
        protected override void OnMouseLeave(EventArgs e) { _hover = false; Invalidate(); base.OnMouseLeave(e); }

        protected override void OnClick(EventArgs e)
        {
            base.OnClick(e);
            try
            {
                if (!string.IsNullOrEmpty(Url)) Native.OpenUrl(Url);
                if (Clicked != null) Clicked(this, EventArgs.Empty);
            }
            catch { }
        }

        protected override void OnPaint(PaintEventArgs e)
        {
            Graphics g = e.Graphics;
            g.SmoothingMode = SmoothingMode.AntiAlias;
            Rectangle r = new Rectangle(0, 0, Width - 1, Height - 1);
            Draw.FillRound(g, r, Height / 2, _hover ? Theme.Hover : Theme.SurfaceAlt);
            Draw.DrawRound(g, r, Height / 2, _hover ? Theme.BorderStrong : Theme.Border, 1f);
            using (SolidBrush b = new SolidBrush(_dot))
            {
                g.FillEllipse(b, 10, Height / 2 - 5, 10, 10);
            }
            TextRenderer.DrawText(g, Text, Font, new Rectangle(26, 0, Width - 34, Height), Theme.Text,
                TextFormatFlags.Left | TextFormatFlags.VerticalCenter | TextFormatFlags.NoPrefix);
        }
    }
    /// <summary>Строка результата проверки.</summary>
    public class CheckRow : Panel
    {
        public CheckResult Result;
        public RoundButton ActionButton;

        public CheckRow(CheckResult result, Action<string> action)
        {
            Result = result;
            Height = 58;
            BackColor = Color.Transparent;
            DoubleBuffered = true;
            Paint += OnPaintRow;
            if (result.ActionId.Length > 0 && result.ActionText.Length > 0)
            {
                ActionButton = new RoundButton();
                ActionButton.Text = result.ActionText;
                ActionButton.Kind = ButtonKind.Secondary;
                ActionButton.Height = 26;
                ActionButton.Fit(10);
                ActionButton.Anchor = AnchorStyles.Top | AnchorStyles.Right;
                ActionButton.Location = new Point(Width - ActionButton.Width - 4, 16);
                ActionButton.Click += delegate { if (action != null) action(Result.ActionId); };
                Controls.Add(ActionButton);
            }
            Resize += delegate
            {
                if (ActionButton != null) ActionButton.Location = new Point(Width - ActionButton.Width - 4, 16);
            };
        }

        private void OnPaintRow(object sender, PaintEventArgs e)
        {
            Graphics g = e.Graphics;
            g.SmoothingMode = SmoothingMode.AntiAlias;
            Color dot = Result.Severity == CheckSeverity.Error ? Theme.Danger : (Result.Severity == CheckSeverity.Warn ? Theme.Warn : Theme.Success);
            using (SolidBrush b = new SolidBrush(dot))
            {
                g.FillEllipse(b, 4, 14, 9, 9);
            }
            int textWidth = Width - 30 - (ActionButton != null ? ActionButton.Width + 12 : 0);
            TextRenderer.DrawText(g, Result.Name, Theme.Bold, new Rectangle(22, 6, textWidth, 18), Theme.Text, TextFormatFlags.Left | TextFormatFlags.NoPrefix | TextFormatFlags.EndEllipsis);
            TextRenderer.DrawText(g, Result.Message, Theme.Small, new Rectangle(22, 24, textWidth, 16), Theme.TextDim, TextFormatFlags.Left | TextFormatFlags.NoPrefix | TextFormatFlags.EndEllipsis);
            if (Result.Hint.Length > 0)
            {
                TextRenderer.DrawText(g, Result.Hint, Theme.Small, new Rectangle(22, 40, textWidth, 16), Theme.TextMuted, TextFormatFlags.Left | TextFormatFlags.NoPrefix | TextFormatFlags.EndEllipsis);
            }
        }
    }
}
