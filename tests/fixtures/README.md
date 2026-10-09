# Video fixtures

These small synthetic clips were generated with FFmpeg's `testsrc2` and `sine`
sources. Each contains six 64×48 frames at 10 fps, with a keyframe followed by
inter frames, and a 48 kHz Vorbis soundtrack. VP8 has 0.6 seconds of mono audio;
VP9 has 0.2 seconds of stereo audio to verify silence after a soundtrack ends.
The reference PNGs contain
all six frames decoded by FFmpeg, tiled horizontally. Asset decoder tests use
these committed fixtures without FFmpeg. CLI conversion tests require FFmpeg
and ffprobe on PATH.

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

The silent alpha fixtures contain six 64×48 frames at 10 fps. Their alpha
planes have transparent and opaque regions plus an animated gradient, testing
keyframes, inter frames, partial transparency, and decoder reset on looping.
VP9 is lossless; VP8 uses its default quality. Reference PNGs must be decoded
with libvpx because FFmpeg's native VP8/VP9 decoders discard alpha.

```sh
for codec in vp8 vp9; do
  encoder=libvpx
  lossless=
  if [ "$codec" = vp9 ]; then
    encoder=libvpx-vp9
    lossless='-lossless 1'
  fi
  ffmpeg -nostdin -hide_banner -loglevel error -y -f lavfi \
    -i "testsrc2=size=64x48:rate=10:duration=0.6,format=yuva420p,geq=lum='p(X,Y)':cb='p(X,Y)':cr='p(X,Y)':a='if(lt(X,16),0,if(gt(X,47),255,mod(16*X+40*N,256)))'" \
    -c:v "$encoder" -pix_fmt yuva420p $lossless -g 30 -auto-alt-ref 0 \
    "tests/fixtures/$codec-alpha.webm"
  ffmpeg -nostdin -hide_banner -loglevel error -y -c:v "$encoder" \
    -i "tests/fixtures/$codec-alpha.webm" -vf tile=6x1 -frames:v 1 \
    "tests/fixtures/$codec-alpha-reference.png"
done
```

Vorbis fixtures for the audio loader are generated from the demo's source WAVs:

```sh
for name in chime theme; do
  ffmpeg -nostdin -hide_banner -loglevel error -y -i "game/audio/$name.wav" \
    -c:a libvorbis -q:a 5 -ac 2 "tests/fixtures/$name.ogg"
done
```

The H.264 MP4 fixtures contain eight 66×50 frames at 10 fps. All three use
two slices per picture and require cropping macroblock padding. Baseline uses
CAVLC and P-frames; Main and High use CABAC and two B-frames between reference
pictures. The reference PNGs contain all eight frames in presentation order.

```sh
for profile in baseline main high; do
  bframes=2
  if [ "$profile" = baseline ]; then bframes=0; fi
  ffmpeg -f lavfi -i testsrc2=size=66x50:rate=10:duration=0.8 \
    -c:v libx264 -profile:v "$profile" -pix_fmt yuv420p -g 30 -bf "$bframes" \
    -x264-params b-adapt=0:slices=2 -y "tests/fixtures/h264-$profile.mp4"
  ffmpeg -i "tests/fixtures/h264-$profile.mp4" -vf tile=8x1 -frames:v 1 \
    -y "tests/fixtures/h264-$profile-reference.png"
done
```
