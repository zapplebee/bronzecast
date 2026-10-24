# UDP playout validation

Verified locally on 2026-10-23/24 with Docker Desktop on Apple Silicon, the native
Rust renderer, Chrome, and the Office Nest Hub.

- Rust asset/selection tests: 4 passed; release build passed.
- Playout tests: 4 passed (latest-frame expiry, bounded audio, generation reset,
  silence on underrun/stale input, incomplete-frame handling).
- Docker service became healthy without any renderer connected. `ffprobe` found
  H.264 960×540 video and stereo 48kHz AAC audio in the fallback stream.
- Inspected the encoded fallback PNG: the requested RTF card appears with the
  ivory/green palette and horizontal scanlines.
- Started the host renderer: `/api/stream` changed to `live`; inspected encoded
  Mog video and verified the proxied notification/character APIs.
- Stopped the renderer: source changed to `fallback`, while encoder uptime and
  output-frame count continued increasing.
- Started an independent FFmpeg test-pattern + quiet 440Hz tone sender on UDP
  port 5000: live input resumed. Decoded output AAC to PCM and measured nonzero
  RMS (61.11 in signed 16-bit units), confirming actual audio delivery.
- Stopped that sender and resumed the renderer: fallback and recovery worked
  without restarting the output encoder. Existing notification contents restored.
- Chrome playback continued through source replacement without reload (time and
  decoded-frame count advanced, no video error). Fullscreen button successfully
  entered fullscreen on `.stage-wrap` with playback continuing.
- Office Hub restart test kept the same Cast media session:
  - live: `PLAYING`, media time ~11s;
  - renderer stopped, test card: `PLAYING`, media time ~23s;
  - renderer restarted: `PLAYING`, media time ~37s.
  Playout uptime advanced from ~87s to ~101s across recovery.

The test guarantees producer-restart continuity. It does not claim seamless output
across replacement/crash of the playout container or its own encoder. See `PLAYOUT.md`.
