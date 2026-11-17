"""VoiceRemoteControl 新图标生成器（Windows .ico + Android launcher PNG）。

视觉：深空蓝渐变底 + 彩色「流光球体」(与应用内 SiriOrb 同款) + 玻璃高光。
输出：
  appicon/out/foreground.png   1024  前景层（UI 球体 + 高光，透明背景）
  appicon/out/background.png   1024  背景层（深空蓝渐变 + 极光+径向光晕）
  appicon/out/squircle.png     1024  完整图标（squircle 圆角，用于 legacy / .ico）
  appicon/out/square.png       1024  完整图标（正方形，用于 .ico 大尺寸）
  appicon/out/32..192.png      legacy 密度 PNG（正方形）
  appicon/out/vrc.ico          8 尺寸 ICO（托盘/Tauri 窗口）
"""
import math, os
import numpy as np
from PIL import Image, ImageDraw, ImageFilter

S = 1024
CX, CY, R = 512, 512, 305  # 球体中心与半径（留在 108dp 自适应安全区内）

# ---------- 工具：径向渐变圆（带椭圆缩放 + 噪声） ----------
def radial_gradient(size, stops, cx=0.5, cy=0.5, rx=0.5, ry=0.5):
    """stops: [(pos 0..1, (r,g,b,a)), ...]，返回 RGBA 图像"""
    y, x = np.mgrid[0:size, 0:size].astype(np.float32)
    nx = (x / (size - 1) - cx) / rx
    ny = (y / (size - 1) - cy) / ry
    d = np.sqrt(nx * nx + ny * ny)
    d = np.clip(d, 0, 1)
    cols = np.array([np.array([s[1][0], s[1][1], s[1][2], s[1][3]], dtype=np.float32) for s in stops])
    pos = np.array([s[0] for s in stops], dtype=np.float32)
    img = np.zeros((size, size, 4), dtype=np.float32)
    for ch in range(4):
        img[..., ch] = np.interp(d, pos, cols[:, ch])
    return Image.fromarray(np.clip(img, 0, 255).astype(np.uint8), 'RGBA')

def ellipse_mask(size, cx, cy, rx, ry):
    m = Image.new('L', (size, size), 0)
    d = ImageDraw.Draw(m)
    d.ellipse([cx - rx, cy - ry, cx + rx, cy + ry], fill=255)
    return m

def combine_screen(base, layer):
    from PIL import ImageChops
    return ImageChops.screen(base, layer)

def combine_add(base, layer):
    a = np.asarray(base, dtype=np.int16) + np.asarray(layer, dtype=np.int16)
    return Image.fromarray(np.clip(a, 0, 255).astype(np.uint8), 'RGBA')

# ---------- 1. 背景层 ----------
def build_background():
    img = radial_gradient(S, [
        (0.0, (18, 22, 45, 255)),
        (0.5, (12, 15, 32, 255)),
        (0.82, (8, 10, 22, 255)),
        (1.0, (4, 5, 11, 255)),
    ], cx=0.44, cy=0.38, rx=0.92, ry=0.92)
    ov = Image.new('RGBA', (S, S), (0, 0, 0, 0))
    d = ImageDraw.Draw(ov)
    # 极淡的品牌极光：左上青紫、右下葡萄紫，压低饱和与亮度，避免虹光
    d.ellipse([-320, -330, 560, 400], fill=(58, 172, 205, 15))
    d.ellipse([420, 560, 1410, 1460], fill=(110, 62, 188, 15))
    d.ellipse([700, -340, 1450, 470], fill=(44, 76, 144, 13))
    ov = ov.filter(ImageFilter.GaussianBlur(210))
    img = combine_screen(img, ov)
    return img

# ---------- 2. 前景层（流光球体） ----------
def build_orb_body():
    body = radial_gradient(S, [
        (0.0, (88, 99, 148, 255)),
        (0.3, (50, 59, 104, 255)),
        (0.62, (27, 33, 65, 255)),
        (0.88, (14, 18, 41, 255)),
        (1.0, (8, 10, 23, 255)),
    ], cx=0.40, cy=0.36, rx=0.46, ry=0.46)
    body.putalpha(ellipse_mask(S, CX, CY, R, R))
    return body

def build_blobs():
    blob_layer = Image.new('RGBA', (S, S), (0, 0, 0, 0))
    d = ImageDraw.Draw(blob_layer)
    cx, cy = CX, CY
    blobs = [
        (cx - 118, cy - 118, 150, 112, (0, 196, 238, 205)),   # cyan
        (cx - 148, cy + 42, 138, 155, (96, 62, 226, 198)),    # violet
        (cx + 132, cy - 28, 152, 148, (232, 52, 138, 198)),   # magenta
        (cx + 78, cy + 158, 128, 102, (240, 158, 40, 196)),   # orange
        (cx - 84, cy + 168, 122, 96, (0, 204, 128, 190)),     # green
    ]
    for x, y, rx, ry, c in blobs:
        tmp = Image.new('RGBA', (S, S), (0, 0, 0, 0))
        dt = ImageDraw.Draw(tmp)
        dt.ellipse([x - rx, y - ry, x + rx, y + ry], fill=c)
        blob_layer = combine_screen(blob_layer, tmp.filter(ImageFilter.GaussianBlur(72)))
    # 限制在球体范围内
    blob_layer.putalpha(Image.composite(
        blob_layer.getchannel('A'),
        Image.new('L', (S, S), 0),
        ellipse_mask(S, CX, CY, R, R),
    ))
    return blob_layer

def build_inner_shadow():
    """球体内底部压暗，增强立体感"""
    sh = radial_gradient(S, [
        (0.0, (0, 0, 0, 0)),
        (0.72, (0, 0, 0, 0)),
        (0.97, (4, 6, 15, 150)),
        (1.0, (4, 6, 15, 190)),
    ], cx=0.5, cy=0.5, rx=0.36, ry=0.36)
    sh.putalpha(Image.composite(
        sh.getchannel('A'),
        Image.new('L', (S, S), 0),
        ellipse_mask(S, CX, CY, R, R),
    ))
    return sh

def build_specular():
    """左上高光（玻璃感核心）"""
    spec = Image.new('RGBA', (S, S), (0, 0, 0, 0))
    d = ImageDraw.Draw(spec)
    d.ellipse([CX - 200, CY - 240, CX - 15, CY - 100], fill=(255, 255, 255, 220))
    spec = spec.filter(ImageFilter.GaussianBlur(46))
    d2 = ImageDraw.Draw(spec)
    d2.ellipse([CX - 82, CY - 182, CX - 8, CY - 136], fill=(255, 255, 255, 190))
    spec = spec.filter(ImageFilter.GaussianBlur(17))
    spec.putalpha(Image.composite(
        spec.getchannel('A'),
        Image.new('L', (S, S), 0),
        ellipse_mask(S, CX, CY, R, R),
    ))
    return spec

def build_rim():
    rim = Image.new('RGBA', (S, S), (0, 0, 0, 0))
    d = ImageDraw.Draw(rim)
    d.ellipse([CX - R, CY - R, CX + R, CY + R], outline=(255, 255, 255, 90), width=10)
    rim = rim.filter(ImageFilter.GaussianBlur(6))
    # 外发光（青色，向外扩散）
    glow = Image.new('RGBA', (S, S), (0, 0, 0, 0))
    dg = ImageDraw.Draw(glow)
    dg.ellipse([CX - R, CY - R, CX + R, CY + R], outline=(0, 213, 255, 90), width=22)
    glow = glow.filter(ImageFilter.GaussianBlur(48))
    out = combine_screen(glow, rim)
    return out

def build_foreground():
    fg = Image.new('RGBA', (S, S), (0, 0, 0, 0))
    fg = combine_add(fg, build_blobs())
    fg = Image.alpha_composite(build_orb_body(), build_inner_shadow())
    fg = Image.alpha_composite(fg, build_blobs())
    fg = Image.alpha_composite(fg, build_specular())
    fg = Image.alpha_composite(fg, build_rim())
    # 整体裁回圆形外轮廓，避免 rim 溢出
    m = ellipse_mask(S, CX, CY, R + 18, R + 18)
    fg.putalpha(Image.composite(fg.getchannel('A'), Image.new('L', (S, S), 0), m))
    return fg

# ---------- 3. 合成：背景 + 前景 ----------
def composite():
    bg = build_background()
    fg = build_foreground()
    full = Image.alpha_composite(bg, fg)
    # squircle（legacy launcher / .ico）：圆角矩形裁切
    squircle = full.copy()
    mask = Image.new('L', (S, S), 0)
    ImageDraw.Draw(mask).rounded_rectangle([0, 0, S - 1, S - 1], radius=232, fill=255)
    squircle = Image.composite(squircle, Image.new('RGBA', (S, S), (0, 0, 0, 0)), mask)
    return bg, fg, full, squircle

def main():
    os.makedirs('appicon/out', exist_ok=True)
    bg, fg, full, squircle = composite()
    bg.save('appicon/out/background.png')
    fg.save('appicon/out/foreground.png')
    full.save('appicon/out/square.png')
    squircle.save('appicon/out/squircle.png')
    for px, name in [(48, 'mipmap-mdpi'), (72, 'mipmap-hdpi'), (96, 'mipmap-xhdpi'),
                     (144, 'mipmap-xxhdpi'), (192, 'mipmap-xxxhdpi')]:
        full.resize((px, px), Image.LANCZOS).save(f'appicon/out/{px}.png')
    # 多尺寸 ICO：16 24 32 48 64 128 256
    squircle.save('appicon/out/vrc.ico', format='ICO',
                  sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
    # tauri 图标
    for px in (32, 128, 256):
        squircle.resize((px, px), Image.LANCZOS).save(f'appicon/out/tauri-{px}.png')
    print('done')

if __name__ == '__main__':
    main()
