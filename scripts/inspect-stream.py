"""Record a whole light sweep and extract lossless review images from the encoded feed.

Usage: python3 scripts/inspect-stream.py [http://localhost:8080/stream/live.m3u8]
Requires ffmpeg. Outputs to scratch/visual-review/<timestamp>/.
"""
import datetime
import pathlib
import subprocess
import sys

url = sys.argv[1] if len(sys.argv) > 1 else 'http://localhost:8080/stream/live.m3u8'
output = pathlib.Path('scratch/visual-review') / datetime.datetime.now().strftime('%Y%m%d-%H%M%S')
output.mkdir(parents=True, exist_ok=False)

def ffmpeg(*args):
    subprocess.run(['ffmpeg', '-hide_banner', '-loglevel', 'error', '-y', *map(str, args)], check=True)

# Capture the actual delivery codec, not a clean GPU screenshot that hides compression.
clip = output / 'cycle.mkv'
ffmpeg('-live_start_index', '-1', '-i', url, '-t', '32', '-an', '-c:v', 'copy', clip)
for index in range(8):
    time = index * 4
    ffmpeg('-ss', time, '-i', clip, '-frames:v', '1', output / f'frame-{index:02}.png')
    ffmpeg('-i', output / f'frame-{index:02}.png', '-vf',
           'scale=1920:1080:flags=neighbor',
           '-frames:v', '1', output / f'scene-{index:02}.png')
    ffmpeg('-i', output / f'frame-{index:02}.png', '-vf',
           'crop=960:140:0:400,scale=1920:280:flags=neighbor',
           '-frames:v', '1', output / f'floor-{index:02}.png')
ffmpeg('-framerate', '1', '-i', output / 'frame-%02d.png', '-vf',
       'scale=480:270:flags=neighbor,tile=4x2', '-frames:v', '1', output / 'contact-sheet.png')
print(f'Review: {output.resolve()}')
print('Frames 00–07 correspond to 0, 4, 8, …, 28 seconds of the captured sweep.')
print('Use full frames and enlarged crops for pixel inspection; contact sheet for lighting/motion.')
