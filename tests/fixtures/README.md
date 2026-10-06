# WebM fixtures

These small synthetic clips were generated with FFmpeg's `testsrc2` and `sine`
sources. Each contains six 64×48 frames at 10 fps, with a keyframe followed by
inter frames, and a 48 kHz Vorbis soundtrack. VP8 has 0.6 seconds of mono audio;
VP9 has 0.2 seconds of stereo audio to verify silence after a soundtrack ends.
The reference PNGs contain
all six frames decoded by FFmpeg, tiled horizontally. FFmpeg is only needed
to regenerate fixtures, not to build, run or test Deflorta.

```sh
for codec in vp8 vp9; do
  encoder=libvpx
  audio_duration=0.6
  channels=1
  if [ "$codec" = vp9 ]; then
    encoder=libvpx-vp9
    audio_duration=0.2
    channels=2
  fi
  ffmpeg -f lavfi -i testsrc2=size=64x48:rate=10:duration=0.6 \
    -f lavfi -i "sine=frequency=440:sample_rate=48000:duration=$audio_duration" \
    -c:v "$encoder" -g 30 -c:a libvorbis -ac "$channels" -y "tests/fixtures/$codec-vorbis.webm"
  ffmpeg -i "tests/fixtures/$codec-vorbis.webm" -vf tile=6x1 -frames:v 1 \
    -y "tests/fixtures/$codec-reference.png"
done
```
