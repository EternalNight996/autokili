
# -*- coding: utf-8 -*-
"""OpenKill app icon: pop-art crossed swords, generated as PNG + ICO + SVG."""
import io, math, os, struct
from PIL import Image, ImageDraw

S = 1024
CREAM = (255, 243, 214)
YELLOW = (255, 216, 77)
RED = (212, 42, 37)
INK = (17, 17, 17)
BLADE = (246, 246, 246)

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
ASSETS = os.path.join(ROOT, "assets", "app-icon")

def rot(p, ang, cx=512, cy=512):
    a = math.radians(ang)
    x, y = p
    dx, dy = x - cx, y - cy
    return (cx + dx * math.cos(a) - dy * math.sin(a), cy + dx * math.sin(a) + dy * math.cos(a))

def star(cx, cy, n=14, outer=322, inner=236):
    pts = []
    for i in range(n * 2):
        ang = math.pi * i / n - math.pi / 2
        r = outer if i % 2 == 0 else inner
        pts.append((cx + r * math.cos(ang), cy + r * math.sin(ang)))
    return pts

def outline(d, pts, w=13):
    d.line(pts + [pts[0]], fill=INK, width=w, joint="curve")

def sword(d, center, angle):
    def P(x, y):
        return rot((center[0] + x, center[1] + y), angle, center[0], center[1])
    blade = [P(-27, -70), P(-27, -352), P(0, -432), P(27, -352), P(27, -70)]
    guard = [P(-98, -70), P(-98, -36), P(98, -36), P(98, -70)]
    grip = [P(-18, -36), P(-18, 74), P(18, 74), P(18, -36)]
    d.polygon([(x + 12, y + 12) for x, y in blade], fill=(0, 0, 0, 80))
    d.polygon(blade, fill=BLADE); outline(d, blade)
    d.polygon(guard, fill=YELLOW); outline(d, guard)
    d.polygon(grip, fill=RED); outline(d, grip)
    cx, cy = P(0, 100)
    d.ellipse([cx - 26, cy - 26, cx + 26, cy + 26], fill=INK)
    d.line([P(0, -368), P(0, -118)], fill=INK, width=7)

img = Image.new("RGBA", (S, S), CREAM + (255,))
d = ImageDraw.Draw(img)
# halftone
dots = Image.new("RGBA", (S, S), (0, 0, 0, 0))
dd = ImageDraw.Draw(dots)
for y in range(30, S, 54):
    for x in range(30, S, 54):
        dist = math.hypot(x - 512, y - 512)
        if dist < 340:
            continue
        r = 4 + (dist / 720.0) * 9
        dd.ellipse([x - r, y - r, x + r, y + r], fill=(212, 42, 37, 70))
img = Image.alpha_composite(img, dots)
d = ImageDraw.Draw(img)
# starburst with hard pop shadow
st = star(512, 512)
d.polygon([(x + 14, y + 14) for x, y in st], fill=(0, 0, 0, 160))
d.polygon(st, fill=YELLOW)
outline(d, st, 12)
# crossed swords
sword(d, (512, 512), 42)
sword(d, (512, 512), -42)
# thick frame
d.rectangle([38, 38, S - 39, S - 39], outline=INK, width=30)

img = img.convert("RGB")
img.save(os.path.join(ASSETS, "craft-anything-1024.png"))

sizes = [(16,16),(24,24),(32,32),(48,48),(64,64),(128,128),(256,256),(512,512)]
for w, h in sizes:
    out = os.path.join(ASSETS, "hicolor", "%dx%d" % (w, h), "apps", "ai.storyteller.craft-anything.png")
    os.makedirs(os.path.dirname(out), exist_ok=True)
    img.resize((w, h), Image.LANCZOS).save(out)
img.save(os.path.join(ASSETS, "craft-anything.ico"), sizes=[(16,16),(24,24),(32,32),(48,48),(64,64),(128,128),(256,256)])

def write_icns(path, image):
    entries = [(b"icp4",16),(b"icp5",32),(b"icp6",64),(b"ic07",128),(b"ic08",256),
               (b"ic09",512),(b"ic10",1024),(b"ic11",32),(b"ic12",64),(b"ic13",256),(b"ic14",512)]
    chunks = []
    for tag, size in entries:
        buf = io.BytesIO()
        image.resize((size, size), Image.LANCZOS).save(buf, format="PNG")
        data = buf.getvalue()
        chunks.append(tag + struct.pack(">I", len(data) + 8) + data)
    body = b"".join(chunks)
    open(path, "wb").write(b"icns" + struct.pack(">I", len(body) + 8) + body)

write_icns(os.path.join(ASSETS, "craft-anything.icns"), img)
print("png/ico/icns written")

def svg_poly(pts, fill, stroke="#111", w=13):
    p = " ".join("%.1f,%.1f" % (x, y) for x, y in pts)
    return '<polygon points="%s" fill="%s" stroke="%s" stroke-width="%d" stroke-linejoin="round"/>' % (p, fill, stroke, w)

parts = ['<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024">',
         '<rect width="1024" height="1024" fill="#fff3d6"/>',
         '<rect x="38" y="38" width="948" height="948" fill="none" stroke="#111" stroke-width="30"/>']
st = star(512, 512)
parts.append('<polygon points="%s" fill="rgba(0,0,0,0.6)"/>' % " ".join("%.1f,%.1f" % (x+14, y+14) for x, y in st))
parts.append(svg_poly(st, "#ffd84d", "#111", 12))
for ang in (42, -42):
    def P(x, y):
        return rot((512 + x, 512 + y), ang, 512, 512)
    blade = [P(-27,-70),P(-27,-352),P(0,-432),P(27,-352),P(27,-70)]
    guard = [P(-98,-70),P(-98,-36),P(98,-36),P(98,-70)]
    grip = [P(-18,-36),P(-18,74),P(18,74),P(18,-36)]
    parts.append(svg_poly(blade, "#f6f6f6"))
    parts.append(svg_poly(guard, "#ffd84d"))
    parts.append(svg_poly(grip, "#d42a25"))
    cx, cy = P(0, 100)
    parts.append('<circle cx="%.1f" cy="%.1f" r="26" fill="#111"/>' % (cx, cy))
parts.append("</svg>")
svg = "\n".join(parts)
for rel in ["craft-anything.svg", os.path.join("hicolor", "scalable", "apps", "ai.storyteller.craft-anything.svg")]:
    out = os.path.join(ASSETS, rel)
    os.makedirs(os.path.dirname(out), exist_ok=True)
    open(out, "w", encoding="utf-8").write(svg)
print("svg written")
