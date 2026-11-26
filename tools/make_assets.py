"""Сборка ассетов приложения из исходных изображений логотипа.

Использование:
    python tools/make_assets.py

Ожидает исходники в assets/source/:
    icon.png  — квадратная иконка (скруглённый квадрат с замком)
    wide.png  — широкий логотип с надписью Zapret Manager

На выходе:
    assets/app-icon.png         256x256  (встраивается в exe, показывается в интерфейсе)
    assets/app-icon-48.png       48x48
    assets/logo-wide.png        800x300  (встраивается в exe, страница «Инструменты»)
    assets/zapret-manager.ico   многоразмерная иконка для exe
"""
import os
from PIL import Image

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOT, "assets", "source")
OUT = os.path.join(ROOT, "assets")

ICON_SIZES = [256, 128, 96, 64, 48, 40, 32, 24, 20, 16]


def trim(im):
    bbox = im.getbbox()
    return im.crop(bbox) if bbox else im


def square_with_margin(im, size, margin=0.03):
    """Вписывает изображение в квадрат с прозрачным полем по краям."""
    im = trim(im).convert("RGBA")
    target = int(size * (1 - margin * 2))
    w, h = im.size
    scale = min(target / w, target / h)
    nw, nh = max(1, int(round(w * scale))), max(1, int(round(h * scale)))
    resized = im.resize((nw, nh), Image.LANCZOS)
    canvas = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    canvas.paste(resized, ((size - nw) // 2, (size - nh) // 2), resized)
    return canvas


def fit_width(im, width):
    im = trim(im)
    w, h = im.size
    height = max(1, int(round(h * width / w)))
    return im.resize((width, height), Image.LANCZOS)


def main():
    icon_src = os.path.join(SRC, "icon.png")
    wide_src = os.path.join(SRC, "wide.png")
    if not os.path.exists(icon_src):
        raise SystemExit("нет файла " + icon_src)
    icon = Image.open(icon_src).convert("RGBA")

    base = square_with_margin(icon, 256)
    base.save(os.path.join(OUT, "app-icon.png"))
    square_with_margin(icon, 48).save(os.path.join(OUT, "app-icon-48.png"))

    images = [base.resize((s, s), Image.LANCZOS) for s in ICON_SIZES]
    ico_path = os.path.join(OUT, "zapret-manager.ico")
    images[0].save(ico_path, format="ICO", sizes=[(s, s) for s in ICON_SIZES])
    print("icon:", os.path.getsize(ico_path), "bytes")

    lock_src = os.path.join(SRC, "lock.png")
    if os.path.exists(lock_src):
        lock = Image.open(lock_src).convert("RGBA")
        l = trim(lock)
        w, h = l.size
        scale = 256.0 / max(w, h)
        l = l.resize((max(1, int(round(w * scale))), max(1, int(round(h * scale)))), Image.LANCZOS)
        l.save(os.path.join(OUT, "lock.png"))
        print("lock:", l.size, os.path.getsize(os.path.join(OUT, "lock.png")), "bytes")

    if os.path.exists(wide_src):
        wide = Image.open(wide_src).convert("RGBA")
        logo = fit_width(wide, 800)
        logo.save(os.path.join(OUT, "logo-wide.png"))
        print("logo:", logo.size, os.path.getsize(os.path.join(OUT, "logo-wide.png")), "bytes")
    print("assets ready")


if __name__ == "__main__":
    main()
