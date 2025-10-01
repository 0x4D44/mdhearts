from PIL import Image, ImageDraw, ImageFont, ImageFilter
from pathlib import Path

BASE_SIZE = 512
BACKGROUND = (6, 35, 27, 255)
HEART_COLOR = (198, 32, 58, 255)

img = Image.new("RGBA", (BASE_SIZE, BASE_SIZE), BACKGROUND)
draw = ImageDraw.Draw(img)

# subtle radial glow
for r in range(0, BASE_SIZE // 2, 8):
    alpha = max(0, 120 - int(r * 0.6))
    if alpha <= 0:
        continue
    bbox = [
        BASE_SIZE / 2 - r,
        BASE_SIZE / 2 - r * 0.78,
        BASE_SIZE / 2 + r,
        BASE_SIZE / 2 + r * 0.78,
    ]
    draw.ellipse(bbox, fill=(255, 238, 180, alpha))

# card shadow
card_rect = [BASE_SIZE * 0.26, BASE_SIZE * 0.12, BASE_SIZE * 0.80, BASE_SIZE * 0.90]
shadow = Image.new("RGBA", img.size, (0, 0, 0, 0))
shadow_draw = ImageDraw.Draw(shadow)
shadow_offset = (18, 22)
shadow_rect = [
    card_rect[0] + shadow_offset[0],
    card_rect[1] + shadow_offset[1],
    card_rect[2] + shadow_offset[0],
    card_rect[3] + shadow_offset[1],
]
shadow_draw.rounded_rectangle(shadow_rect, radius=BASE_SIZE * 0.08, fill=(0, 0, 0, 120))
shadow = shadow.filter(ImageFilter.GaussianBlur(20))
img = Image.alpha_composite(img, shadow)
draw = ImageDraw.Draw(img)

# card body
draw.rounded_rectangle(card_rect, radius=BASE_SIZE * 0.08, fill=(247, 247, 248, 255), outline=(223, 204, 137), width=12)
inner_rect = [card_rect[0] + 18, card_rect[1] + 18, card_rect[2] - 18, card_rect[3] - 18]
draw.rounded_rectangle(inner_rect, radius=BASE_SIZE * 0.06, outline=(200, 215, 220), width=4)

# heart glyph
def load_font(path, size):
    try:
        return ImageFont.truetype(path, size)
    except OSError:
        return None

font_paths = [
    "C:/Windows/Fonts/seguisym.ttf",
    "C:/Windows/Fonts/segoeuib.ttf",
    "C:/Windows/Fonts/segoeui.ttf",
]
font = None
for path in font_paths:
    font = load_font(path, int(BASE_SIZE * 0.42))
    if font:
        break
if font is None:
    raise SystemExit("Unable to locate a usable font for heart glyph")

heart = "\u2665"
text_bbox = draw.textbbox((0, 0), heart, font=font)
text_w = text_bbox[2] - text_bbox[0]
text_h = text_bbox[3] - text_bbox[1]
card_center = ((card_rect[0] + card_rect[2]) / 2, (card_rect[1] + card_rect[3]) / 2)
text_pos = (card_center[0] - text_w / 2, card_center[1] - text_h / 2 - BASE_SIZE * 0.02)
draw.text(text_pos, heart, font=font, fill=HEART_COLOR)

# smaller corner hearts
pip_font = ImageFont.truetype(font.path, int(BASE_SIZE * 0.14))
pip_positions = [
    (inner_rect[0] + 22, inner_rect[1] + 12),
    (inner_rect[2] - 60, inner_rect[3] - 60),
]
for pos in pip_positions:
    draw.text(pos, heart, font=pip_font, fill=HEART_COLOR)

# translucent ribbon band
ribbon_rect = [card_rect[0], card_center[1] - 12, card_rect[2], card_center[1] + 24]
draw.rectangle(ribbon_rect, fill=(239, 92, 117, 90))
draw.rectangle([ribbon_rect[0], ribbon_rect[1] - 8, ribbon_rect[2], ribbon_rect[1]], fill=(255, 255, 255, 50))
draw.rectangle([ribbon_rect[0], ribbon_rect[3], ribbon_rect[2], ribbon_rect[3] + 6], fill=(0, 0, 0, 35))

# sparkle accents
spark = Image.new("RGBA", img.size, (0, 0, 0, 0))
spark_draw = ImageDraw.Draw(spark)
for cx, cy, r in [(inner_rect[0] + 40, inner_rect[1] + 60, 24), (inner_rect[2] - 60, inner_rect[1] + 90, 18), (card_center[0], inner_rect[3] - 80, 16)]:
    for i in range(3):
        alpha = max(0, 110 - i * 36)
        if alpha <= 0:
            continue
        spark_draw.ellipse([cx - (r - i * 5), cy - (r - i * 5), cx + (r - i * 5), cy + (r - i * 5)], outline=(255, 255, 255, alpha), width=2)
img = Image.alpha_composite(img, spark)

out_dir = Path('res')
out_dir.mkdir(exist_ok=True)
icon_path = out_dir / 'app.ico'
img.save(icon_path, format='ICO', sizes=[(256,256),(128,128),(96,96),(64,64),(48,48),(32,32),(24,24),(16,16)])
print(f"Wrote {icon_path}")
