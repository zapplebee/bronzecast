import io
import unittest
from unittest.mock import patch

from playout.server import MediaBuffer, AUDIO_BYTES, FRAME_BYTES, read_exact


class BufferTests(unittest.TestCase):
    def test_latest_video_expires_to_fallback(self):
        buffer = MediaBuffer(timeout=1.5)
        generation = buffer.reset()
        first, latest, fallback = bytes(FRAME_BYTES), b'\x01' * FRAME_BYTES, b'card'
        with patch('playout.server.time.monotonic', return_value=10):
            buffer.video(generation, first)
            buffer.video(generation, latest)
            self.assertEqual(buffer.output_video(fallback), latest)
        with patch('playout.server.time.monotonic', return_value=11.6):
            self.assertEqual(buffer.output_video(fallback), fallback)
            self.assertEqual(buffer.status()['source'], 'fallback')

    def test_audio_is_bounded_and_old_sessions_cannot_leak(self):
        buffer = MediaBuffer()
        generation = buffer.reset()
        with patch('playout.server.time.monotonic', return_value=10):
            buffer.video(generation, bytes(FRAME_BYTES))
            for i in range(20):
                buffer.sound(generation, bytes([i]) * AUDIO_BYTES)
            self.assertEqual(buffer.status()['audio_buffer_ms'], 200)
            self.assertEqual(buffer.output_audio(), bytes([10]) * AUDIO_BYTES)
            buffer.reset()
            buffer.sound(generation, b'\x01' * AUDIO_BYTES)
            buffer.video(generation, bytes(FRAME_BYTES))
            self.assertEqual(buffer.output_audio(), bytes(AUDIO_BYTES))
            self.assertEqual(buffer.status()['source'], 'fallback')

    def test_audio_underflow_and_expired_input_use_silence(self):
        buffer = MediaBuffer()
        generation = buffer.reset()
        with patch('playout.server.time.monotonic', return_value=10):
            buffer.video(generation, bytes(FRAME_BYTES))
            self.assertEqual(buffer.output_audio(), bytes(AUDIO_BYTES))
            buffer.sound(generation, b'\x01' * AUDIO_BYTES)
        with patch('playout.server.time.monotonic', return_value=20):
            self.assertEqual(buffer.output_audio(), bytes(AUDIO_BYTES))

    def test_incomplete_media_is_not_published(self):
        buffer = MediaBuffer()
        with self.assertRaises(ValueError):
            buffer.video(0, b'incomplete')
        with self.assertRaises(ValueError):
            buffer.sound(0, b'incomplete')
        self.assertIsNone(read_exact(io.BytesIO(b'ab'), 3))
        self.assertEqual(read_exact(io.BytesIO(b'abcd'), 3), b'abc')


if __name__ == '__main__':
    unittest.main()
