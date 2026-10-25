# Independent UDP playout

The host GPU renderer is a replaceable producer. Docker owns the long-running
web player, output encoder, and HLS timeline.

The proxy exposes unified sticker CRUD routes and the read-only rendered snapshot.
Notification and clock behaviors use the same sticker API, persistent store, and
chosen appearance list. Art stickers use custom SVG through the same API.

```text
Host: Rust renderer + FFmpeg sender
       H.264 + AAC in MPEG-TS
                  |
             UDP :5000
                  v
Docker: decoder -> latest video / short audio buffer
                            |
                 fixed 12fps / 48kHz clock
                 live frames OR RTF test card
                 audio samples OR silence
                            |
                  persistent H.264/AAC encoder
                            |
                  HTTP :8080 /stream/live.m3u8
```

## Start

```sh
docker compose up -d --build
cargo run --release --locked
```

Open `http://localhost:8080` or the host's LAN address on port 8080. The test card
plays even before the renderer starts. Initial HLS buffering takes several seconds.
The browser starts muted; click **Unmute** to hear incoming audio.

## Iterate on the renderer

Keep Compose running. Stop the native renderer (Ctrl+C), edit/rebuild, and run it
again. The test card appears automatically while it is absent. The playout encoder
does not restart and the playlist URL stays the same; clients do not need a reload
or a new Cast session. To stop the whole system, stop the renderer and run
`docker compose down`.

This guarantee is for **producer restarts**. Rebuilding/recreating the `playout`
container still restarts the output encoder. Do that only when changing playout
itself; the host renderer does not need the container rebuilt.

## Ports and settings

| Setting | Default | Purpose |
|---|---|---|
| `BRONZECAST_UDP_URL` (renderer env) | `udp://127.0.0.1:5000?pkt_size=1316` | Destination for MPEG-TS output; point to another host or service name later. |
| `BRONZECAST_CONTROL_ADDR` (renderer env) | `0.0.0.0:8081` | Generator's HTTP control API. |
| `BRONZECAST_AUDIO_FILE` (renderer env) | unset | Optional looping audio file; otherwise sends AAC silence. This provides audio input, not notification chimes/TTS. |
| `BRONZECAST_STATE_DIR` (renderer env) | `state` | Where the sticker store (`stickers.json`) lives. Stickers survive renderer restarts; notification stickers expire at their agent-supplied TTL. |
| `GENERATOR_URL` (Compose env) | `http://host.docker.internal:8081` | API upstream for character, sticker, and snapshot requests. |
| `BRONZECAST_WRITE_KEY` (playout env) | unset | When set, public `POST`/`PUT`/`PATCH`/`DELETE /api/*` requests must send it as `Authorization: Bearer <key>` or `X-Bronzecast-Key`. `GET`s stay public. |
| `INPUT_TIMEOUT` (Compose env) | `1.5` seconds | Age of last decoded frame before showing the test card. |
| `LOG_LEVEL` (Compose env) | `INFO` | Use `DEBUG` for input-decoder diagnostics; UDP read timeout while idle is normal. |

Host endpoints remain `GET /api/characters`, `GET/POST /api/character`,
`GET/POST /api/stickers`,
`PATCH/DELETE /api/stickers/{id}`, and `GET /api/snapshot` on port **8080**,
proxied to the generator on 8081. Snapshot returns the latest full renderer frame
as `image/png`; it is read-only and requires no write key.
While the generator is offline these return 503; the video remains available.
Character selection resets to the default on renderer restart. Stickers persist
under `BRONZECAST_STATE_DIR`; notification stickers remain until expiry.

`GET /api/stream` reports `source` (`live` or `fallback`), decoded frame age,
input generation, bounded audio buffer size, output frame count, and encoder
uptime. `/health` checks that the output encoder is running.

## Feed it from FFmpeg instead

**Stop the renderer first. Only one producer should send to the UDP port.**
Input contract: one H.264 video stream and one AAC audio stream in MPEG-TS;
12fps, 960×540 and stereo 48kHz are recommended. Other video dimensions are
letterboxed. Use short GOPs, no B-frames and regularly repeated stream headers.
Silence is a valid audio track; a video-only input is not the current contract.

```sh
ffmpeg -re -f lavfi -i testsrc2=size=960x540:rate=12 \
  -f lavfi -i sine=frequency=440:sample_rate=48000 \
  -af volume=0.03 \
  -c:v libx264 -preset ultrafast -tune zerolatency \
  -pix_fmt yuv420p -g 12 -bf 0 \
  -c:a aac -ar 48000 -ac 2 \
  -mpegts_flags +resend_headers+initial_discontinuity \
  -muxdelay 0 -muxpreload 0 -f mpegts \
  'udp://127.0.0.1:5000?pkt_size=1316'
```

Or play a file in a loop:

```sh
ffmpeg -re -stream_loop -1 -i clip-with-audio.mp4 \
  -vf 'scale=960:540:force_original_aspect_ratio=decrease,pad=960:540:(ow-iw)/2:(oh-ih)/2,fps=12' \
  -c:v libx264 -preset ultrafast -tune zerolatency -pix_fmt yuv420p -g 12 -bf 0 \
  -c:a aac -ar 48000 -ac 2 \
  -mpegts_flags +resend_headers+initial_discontinuity -muxdelay 0 \
  -f mpegts 'udp://127.0.0.1:5000?pkt_size=1316'
```

## Buffering and recovery

- Video is a latest-frame slot, never an unbounded queue of stale frames.
- Audio holds at most 200ms of decoded PCM; underruns output silence.
- A stopped/stalled sender triggers the RTF test card and silence. The input
  decoder can restart/reprobe, with a new generation discarding old samples.
- The output encoder continuously receives video and PCM, including when the
  decoder is absent. It produces a continuous timestamp sequence for clients.
- UDP is best-effort. Input acquisition/recovery can take several seconds while
  waiting for headers and a keyframe. HLS adds its own playback delay.
- Audio/video use a common output clock and bounded arrival buffers; this is a
  lightweight local playout path, not a timestamp-preserving broadcast synchronizer.
- An output encoder crash exits the service so Docker can restart it. That is
  a different failure domain from the deliberately restartable renderer.

## Checks

```sh
docker compose ps
curl http://localhost:8080/api/stream
ffprobe -v error -show_streams http://localhost:8080/stream/live.m3u8
docker compose logs -f playout
docker compose run --rm --no-deps playout python -m unittest playout.test_server
python3 scripts/inspect-stream.py
```

The fallback is based on the user-selected 1953 RTF test card, styled at service
startup with stepped ivory/green tones and even scanlines. The image is bundled,
so fallback does not require an internet connection. See `playout/assets/README.md`.
