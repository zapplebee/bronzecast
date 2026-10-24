"""UDP MPEG-TS ingress -> bounded raw media buffers -> continuous HLS encoder.

The decoder may die/restart; the output encoder never sees input EOF until this
service shuts down. Output clocks keep running on test-card frames and PCM silence.
"""
import collections
import hmac
import http.server
import json
import logging
import math
import mimetypes
import os
import pathlib
import signal
import subprocess
import threading
import time
import urllib.error
import urllib.parse
import urllib.request
from PIL import Image, ImageDraw, ImageFont, ImageOps

WIDTH, HEIGHT, FPS = 960, 540, 12
RATE, CHANNELS, SAMPLE_BYTES = 48000, 2, 2
AUDIO_TICK = 0.02
AUDIO_BYTES = int(RATE * AUDIO_TICK) * CHANNELS * SAMPLE_BYTES
FRAME_BYTES = WIDTH * HEIGHT * 3
LOG = logging.getLogger('playout')


class MediaBuffer:
    """Latest video only; at most 200ms of audio. Never accumulate stale video."""
    def __init__(self, timeout=1.5):
        self.lock = threading.Lock()
        self.timeout = timeout
        self.frame = None
        self.video_at = None
        self.audio_at = None
        self.audio = collections.deque(maxlen=10)
        self.received_frames = 0
        self.generation = 0

    def reset(self):
        with self.lock:
            self.generation += 1
            self.frame = None
            self.video_at = self.audio_at = None
            self.audio.clear()
            return self.generation

    def video(self, generation, frame):
        if len(frame) != FRAME_BYTES:
            raise ValueError('Incomplete frame')
        with self.lock:
            if generation == self.generation:
                self.frame, self.video_at = frame, time.monotonic()
                self.received_frames += 1

    def sound(self, generation, chunk):
        if len(chunk) != AUDIO_BYTES:
            raise ValueError('Incomplete audio block')
        with self.lock:
            if generation == self.generation:
                self.audio.append(chunk)
                self.audio_at = time.monotonic()

    def live(self, now):
        return self.video_at is not None and now - self.video_at < self.timeout

    def output_video(self, fallback):
        with self.lock:
            return self.frame if self.live(time.monotonic()) else fallback

    def output_audio(self):
        with self.lock:
            now = time.monotonic()
            if not self.live(now) or self.audio_at is None or now - self.audio_at >= self.timeout:
                self.audio.clear()
            return self.audio.popleft() if self.audio else bytes(AUDIO_BYTES)

    def status(self):
        with self.lock:
            now = time.monotonic()
            return {
                'source': 'live' if self.live(now) else 'fallback',
                'video_age_seconds': None if self.video_at is None else round(now - self.video_at, 3),
                'audio_buffer_ms': len(self.audio) * 20,
                'received_frames': self.received_frames,
                'input_generation': self.generation,
            }


def read_exact(stream, length):
    data = bytearray()
    while len(data) < length:
        chunk = stream.read(length - len(data))
        if not chunk:
            return None
        data.extend(chunk)
    return bytes(data)


def write_all(fd, data):
    remaining = memoryview(data)
    while remaining:
        count = os.write(fd, remaining)
        remaining = remaining[count:]


def write_authorized(headers, key):
    """Write requests pass when no key is configured; otherwise a bearer/header match is required."""
    if not key:
        return True
    supplied = headers.get('Authorization', '')
    if supplied.lower().startswith('bearer '):
        supplied = supplied[7:]
    else:
        supplied = headers.get('X-Bronzecast-Key', '')
    return hmac.compare_digest(supplied.strip(), key)


class Playout:
    def __init__(self):
        self.media = MediaBuffer(float(os.getenv('INPUT_TIMEOUT', '1.5')))
        self.stopping = threading.Event()
        self.output = pathlib.Path(os.getenv('HLS_DIR', '/data/hls'))
        self.web = pathlib.Path(os.getenv('WEB_DIR', '/app/web')).resolve()
        self.generator_url = os.getenv('GENERATOR_URL', 'http://host.docker.internal:8081').rstrip('/')
        self.write_key = os.getenv('BRONZECAST_WRITE_KEY', '').strip()
        self.encoder = None
        self.decoder = None
        self.encoder_started = time.monotonic()
        self.output_frames = 0

    def test_card(self):
        # Generated once; a fresh frame still goes into the encoder on every tick.
        source = pathlib.Path(__file__).parent / 'assets/rtf-1953.jpg'
        with Image.open(source) as original:
            card = ImageOps.contain(original.convert('L'), (680, 480), Image.Resampling.NEAREST)
        image = Image.new('RGB', (WIDTH, HEIGHT), (0, 0, 0))
        x0, y0 = (WIDTH - card.width) // 2, (HEIGHT - card.height) // 2
        pixels, gray = image.load(), card.load()
        for y in range(card.height):
            scan = 0.72 + 0.28 * math.cos(2 * math.pi * (y + y0 + 0.5) / 4)
            for x in range(card.width):
                level = round(gray[x, y] / 255 * 6) / 6
                value = int(235 * level * scan)
                pixels[x + x0, y + y0] = (int(value * 0.96), value, int(value * 0.88))
        draw = ImageDraw.Draw(image)
        font = ImageFont.truetype('/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf', 15)
        draw.text((24, 7), 'BRONZECAST / STANDBY', fill=(157, 208, 104), font=font)
        draw.text((24, HEIGHT - 24), 'NO VIDEO INPUT   /   WAITING FOR GENERATOR', fill=(157, 208, 104), font=font)
        draw.text((WIDTH - 150, HEIGHT - 24), 'UDP :5000', fill=(120, 147, 108), font=font)
        return image.tobytes()

    def start(self):
        self.output.mkdir(parents=True, exist_ok=True)
        # Only our generated media lives here; avoid accumulating old encoder runs.
        for pattern in ('segment-*.ts', 'segment-*.ts.tmp', 'live.m3u8', 'live.m3u8.tmp'):
            for path in self.output.glob(pattern):
                path.unlink()
        fallback = self.test_card()
        video_read, video_write = os.pipe()
        audio_read, audio_write = os.pipe()
        self.encoder = subprocess.Popen([
            'ffmpeg', '-hide_banner', '-loglevel', 'warning', '-nostdin', '-y',
            '-thread_queue_size', '8', '-f', 'rawvideo', '-pix_fmt', 'rgb24',
            '-s', f'{WIDTH}x{HEIGHT}', '-r', str(FPS), '-i', f'pipe:{video_read}',
            '-thread_queue_size', '32', '-probesize', '32', '-analyzeduration', '0',
            '-f', 's16le', '-ar', str(RATE), '-ac', str(CHANNELS),
            '-i', f'pipe:{audio_read}', '-map', '0:v:0', '-map', '1:a:0',
            '-c:v', 'libx264', '-threads', '2', '-preset', 'ultrafast', '-tune', 'zerolatency',
            '-pix_fmt', 'yuv420p', '-g', str(FPS * 2), '-keyint_min', str(FPS * 2),
            '-sc_threshold', '0', '-bf', '0', '-c:a', 'aac', '-b:a', '128k',
            '-f', 'hls', '-hls_time', '2', '-hls_list_size', '8',
            '-hls_delete_threshold', '3', '-hls_start_number_source', 'epoch',
            '-hls_flags', 'delete_segments+independent_segments+temp_file+omit_endlist',
            '-hls_segment_filename', str(self.output / 'segment-%010d.ts'),
            str(self.output / 'live.m3u8'),
        ], pass_fds=(video_read, audio_read))
        os.close(video_read)
        os.close(audio_read)
        self.encoder_started = time.monotonic()
        for target, args in [
            (self.video_clock, (video_write, fallback)),
            (self.audio_clock, (audio_write,)),
            (self.decode_loop, ()),
            (self.encoder_watch, ()),
        ]:
            threading.Thread(target=target, args=args, daemon=True).start()

    def encoder_watch(self):
        code = self.encoder.wait()
        if not self.stopping.is_set():
            LOG.error('Output encoder exited (%s); restarting service is required', code)
            # Let the container restart policy recover; do not serve a frozen playlist as healthy.
            os._exit(1)

    def video_clock(self, fd, fallback):
        deadline = self.encoder_started
        try:
            while not self.stopping.is_set():
                if self.stopping.wait(max(0, deadline - time.monotonic())):
                    break
                write_all(fd, self.media.output_video(fallback))
                self.output_frames += 1
                deadline += 1 / FPS
        except (BrokenPipeError, OSError):
            if not self.stopping.is_set():
                LOG.exception('Video output pipe failed')
                os._exit(1)
        finally:
            os.close(fd)

    def audio_clock(self, fd):
        deadline = self.encoder_started
        try:
            while not self.stopping.is_set():
                if self.stopping.wait(max(0, deadline - time.monotonic())):
                    break
                write_all(fd, self.media.output_audio())
                deadline += AUDIO_TICK
        except (BrokenPipeError, OSError):
            if not self.stopping.is_set():
                LOG.exception('Audio output pipe failed')
                os._exit(1)
        finally:
            os.close(fd)

    def decode_loop(self):
        while not self.stopping.is_set():
            generation = self.media.reset()
            audio_read, audio_write = os.pipe()
            command = [
                'ffmpeg', '-hide_banner', '-loglevel', 'warning', '-nostdin',
                '-fflags', '+discardcorrupt', '-probesize', '131072', '-analyzeduration', '500000',
                '-dts_delta_threshold', '0.5', '-threads', '2',
                '-i', 'udp://0.0.0.0:5000?fifo_size=4096&overrun_nonfatal=1&timeout=1500000',
                '-map', '0:v:0', '-an', '-vf', f'scale={WIDTH}:{HEIGHT}:force_original_aspect_ratio=decrease,pad={WIDTH}:{HEIGHT}:(ow-iw)/2:(oh-ih)/2',
                '-fps_mode', 'passthrough', '-c:v', 'rawvideo', '-threads', '1', '-pix_fmt', 'rgb24',
                '-f', 'rawvideo', 'pipe:1',
                '-map', '0:a:0', '-vn', '-c:a', 'pcm_s16le', '-ar', str(RATE), '-ac', str(CHANNELS),
                '-f', 's16le', f'pipe:{audio_write}',
            ]
            try:
                self.decoder = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, pass_fds=(audio_write,))
            except OSError:
                os.close(audio_read)
                os.close(audio_write)
                raise
            os.close(audio_write)
            reader = threading.Thread(target=self.read_audio, args=(audio_read, generation), daemon=True)
            reader.start()
            errors = threading.Thread(target=self.read_errors, args=(self.decoder.stderr,), daemon=True)
            errors.start()
            try:
                while not self.stopping.is_set():
                    frame = read_exact(self.decoder.stdout, FRAME_BYTES)
                    if frame is None:
                        break
                    self.media.video(generation, frame)
            finally:
                self.decoder.stdout.close()
                if self.decoder.poll() is None:
                    self.decoder.terminate()
                try:
                    self.decoder.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    self.decoder.kill()
                    self.decoder.wait()
                reader.join(timeout=3)
                errors.join(timeout=3)
            self.stopping.wait(0.1)

    def read_audio(self, fd, generation):
        with os.fdopen(fd, 'rb', buffering=0) as pipe:
            while not self.stopping.is_set():
                chunk = read_exact(pipe, AUDIO_BYTES)
                if chunk is None:
                    return
                self.media.sound(generation, chunk)

    @staticmethod
    def read_errors(pipe):
        with pipe:
            for line in pipe:
                # UDP timeout while idle is normal; retain diagnostics at DEBUG.
                LOG.debug('decoder: %s', line.decode(errors='replace').strip())

    def status(self):
        return {
            **self.media.status(),
            'encoder_running': self.encoder is not None and self.encoder.poll() is None,
            'output_frames': self.output_frames,
            'uptime_seconds': round(time.monotonic() - self.encoder_started, 1),
            'width': WIDTH, 'height': HEIGHT, 'fps': FPS, 'audio_rate': RATE,
        }

    def stop(self):
        self.stopping.set()
        for child in (self.decoder, self.encoder):
            if child and child.poll() is None:
                child.terminate()
                try:
                    child.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    child.kill()
                    child.wait()


def handler_for(playout):
    class Handler(http.server.SimpleHTTPRequestHandler):
        def __init__(self, *args, **kwargs):
            super().__init__(*args, directory=str(playout.web), **kwargs)

        def log_message(self, fmt, *args):
            # Playlist polling should not drown out input/encoder errors.
            if args and str(args[1]) not in ('200', '304'):
                LOG.info(fmt, *args)

        def end_headers(self):
            self.send_header('Access-Control-Allow-Origin', '*')
            self.send_header('Access-Control-Allow-Headers', 'Content-Type, Range, Authorization, X-Bronzecast-Key')
            self.send_header('Access-Control-Expose-Headers', 'Content-Length, Content-Range')
            self.send_header('Access-Control-Allow-Methods', 'GET, HEAD, POST, PUT, DELETE, OPTIONS')
            self.send_header('Cache-Control', 'no-store')
            super().end_headers()

        def do_OPTIONS(self):
            self.send_response(204)
            self.end_headers()

        def respond(self, code, body, mime='application/json'):
            self.send_response(code)
            self.send_header('Content-Type', mime)
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            if self.command != 'HEAD':
                self.wfile.write(body)

        def proxy(self):
            path = urllib.parse.urlsplit(self.path).path
            allowed = ('/api/notifications', '/api/characters', '/api/character', '/api/open-item')
            if path not in allowed:
                self.respond(404, b'{"error":"Unknown API endpoint"}')
                return
            if self.command != 'GET' and not write_authorized(self.headers, playout.write_key):
                self.respond(401, b'{"error":"Write key required"}')
                return
            try:
                length = int(self.headers.get('Content-Length', '0'))
            except ValueError:
                self.respond(400, b'{"error":"Invalid Content-Length"}')
                return
            if length < 0 or length > 16384:
                self.respond(413, b'{"error":"Request too large"}')
                return
            payload = self.rfile.read(length) if self.command in ('POST', 'PUT') else None
            request = urllib.request.Request(
                playout.generator_url + path, data=payload, method=self.command,
                headers={'Content-Type': self.headers.get('Content-Type', 'application/json')},
            )
            try:
                with urllib.request.urlopen(request, timeout=2) as response:
                    self.respond(response.status, response.read(), response.headers.get('Content-Type', 'application/json'))
            except urllib.error.HTTPError as error:
                self.respond(error.code, error.read(), error.headers.get('Content-Type', 'text/plain'))
            except (OSError, urllib.error.URLError):
                self.respond(503, b'{"error":"Generator offline; video fallback is still playing"}')

        def translate_path(self, path):
            parsed = urllib.parse.urlsplit(path).path
            if parsed.startswith('/stream/'):
                # HLS consists only of flat files. Never expose arbitrary volume paths.
                name = urllib.parse.unquote(parsed[len('/stream/'):])
                if '/' in name or '\\' in name or name.startswith('.'):
                    return str(playout.output / '__invalid__')
                return str(playout.output / name)
            return super().translate_path(path)

        def do_GET(self):
            path = urllib.parse.urlsplit(self.path).path
            if path in ('/api/stream', '/health'):
                status = playout.status()
                self.respond(200 if status['encoder_running'] else 503, json.dumps(status).encode())
            elif path.startswith('/api/'):
                self.proxy()
            else:
                super().do_GET()

        def do_POST(self):
            self.proxy()

        def do_PUT(self):
            self.proxy()

        def do_DELETE(self):
            self.proxy()

    return Handler


def main():
    logging.basicConfig(level=os.getenv('LOG_LEVEL', 'INFO'), format='%(asctime)s %(levelname)s %(message)s')
    mimetypes.add_type('application/vnd.apple.mpegurl', '.m3u8')
    mimetypes.add_type('video/mp2t', '.ts')
    playout = Playout()
    playout.start()
    server = http.server.ThreadingHTTPServer(('0.0.0.0', 8080), handler_for(playout))
    def shutdown(_signal, _frame):
        threading.Thread(target=server.shutdown, daemon=True).start()
    signal.signal(signal.SIGTERM, shutdown)
    signal.signal(signal.SIGINT, shutdown)
    LOG.info('Web/HLS on :8080, UDP ingest on :5000; control upstream %s', playout.generator_url)
    try:
        server.serve_forever()
    finally:
        server.server_close()
        playout.stop()


if __name__ == '__main__':
    main()
