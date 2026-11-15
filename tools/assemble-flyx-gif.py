"""Assemble image_gen's aligned 3x2 transparent sprite sheet; preserve source art."""
from pathlib import Path
import argparse
from PIL import Image

parser = argparse.ArgumentParser()
parser.add_argument('sheet', type=Path)
parser.add_argument('--output', type=Path, default=Path('output/animations'))
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=True)
sheet = Image.open(args.sheet).convert('RGBA')
if sheet.getchannel('A').getextrema()[0] != 0:
    raise ValueError('Source sheet has no fully transparent pixels')

cells = []
for i in range(6):
    x, y = i % 3, i // 3
    bounds = (round(x * sheet.width / 3), round(y * sheet.height / 2),
              round((x + 1) * sheet.width / 3), round((y + 1) * sheet.height / 2))
    cell = sheet.crop(bounds).resize((512, 512), Image.Resampling.LANCZOS)
    cell.save(args.output / f'flyx-expression-{i + 1}.png')
    cells.append(cell)

# One brief blink, a surprised look, then a cheeky wink; return to rest.
sequence = [0, 1, 2, 1, 0, 3, 4, 3, 0, 1, 2, 1, 5]
durations = [1100, 70, 100, 70, 500, 650, 800, 180, 600, 70, 100, 70, 900]
frames = [cells[i] for i in sequence]

# A shared palette prevents color shimmer. Index 255 is reserved for transparency.
palette_source = Image.new('RGB', (512 * 3, 512 * 2))
for i, frame in enumerate(cells):
    rgb = Image.new('RGB', frame.size)
    rgb.paste(frame, mask=frame.getchannel('A'))
    palette_source.paste(rgb, (i % 3 * 512, i // 3 * 512))
palette = palette_source.quantize(colors=255, method=Image.Quantize.MEDIANCUT)
gif_frames = []
for frame in frames:
    indexed = frame.convert('RGB').quantize(palette=palette, dither=Image.Dither.FLOYDSTEINBERG)
    transparent = frame.getchannel('A').point(lambda a: 255 if a < 128 else 0)
    indexed.paste(255, mask=transparent)
    indexed.info['transparency'] = 255
    gif_frames.append(indexed)

gif_path = args.output / 'flyx-funny-blink-transparent.gif'
gif_frames[0].save(gif_path, save_all=True, append_images=gif_frames[1:],
                   duration=durations, loop=0, disposal=2, transparency=255,
                   optimize=False)
webp_path = args.output / 'flyx-funny-blink-transparent.webp'
frames[0].save(webp_path, save_all=True, append_images=frames[1:],
               duration=durations, loop=0, lossless=True, method=6)

for path in (gif_path, webp_path):
    with Image.open(path) as animation:
        for i in range(animation.n_frames):
            animation.seek(i)
            rgba = animation.convert('RGBA')
            assert rgba.getpixel((0, 0))[3] == 0, (path, i, 'opaque corner')
        print(f'{path.resolve()} | {animation.size} | {animation.n_frames} frames | '
              f'{path.stat().st_size} bytes | transparent corners verified')
