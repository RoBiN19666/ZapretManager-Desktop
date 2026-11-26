"""Генерация иконки приложения (assets/zapret-manager.ico)."""
import os
from PIL import Image, ImageDraw

OUT = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "assets", "zapret-manager.ico")
SIZE = 512

def lerp(a, b, t):
    return tuple(int(round(a[i] + (b[i] - a[i]) * t)) for i in range(3))

def main():
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    img = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))

    # фон: скруглённый квадрат с градиентом
    grad = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    gd = ImageDraw.Draw(grad)
    top = (96, 146, 255)
    bottom = (38, 82, 200)
    for y in range(SIZE):
        gd.line([(0, y), (SIZE, y)], fill=lerp(top, bottom, y / (SIZE - 1)) + (255,))

    mask = Image.new("L", (SIZE, SIZE), 0)
    md = ImageDraw.Draw(mask)
    pad = int(SIZE * 0.03)
    md.rounded_rectangle([pad, pad, SIZE - pad, SIZE - pad], radius=int(SIZE * 0.23), fill=255)
    img.paste(grad, (0, 0), mask)

    d = ImageDraw.Draw(img)

    # лёгкий блик сверху
    gloss = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    gld = ImageDraw.Draw(gloss)
    gld.rounded_rectangle([pad, pad, SIZE - pad, int(SIZE * 0.5)], radius=int(SIZE * 0.21), fill=(255, 255, 255, 26))
    img.alpha_composite(Image.composite(gloss, Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0)), mask))

    # молния
    s = SIZE / 100.0
    bolt = [(58, 8), (26, 55), (46, 55), (36, 92), (74, 40), (52, 40), (63, 8)]
    poly = [(x * s, y * s) for x, y in bolt]
    shift = (0, SIZE * 0.0)
    poly = [(x + shift[0], y + shift[1]) for x, y in poly]

    shadow = [(x + 2.0, y + 3.0) for x, y in poly]
    d.polygon(shadow, fill=(20, 40, 90, 90))
    d.polygon(poly, fill=(255, 255, 255, 255))

    sizes = [(256, 256), (128, 128), (64, 64), (48, 48), (32, 32), (24, 24), (16, 16)]
    img.save(OUT, format="ICO", sizes=sizes)
    print("icon written:", OUT)

if __name__ == "__main__":
    main()
