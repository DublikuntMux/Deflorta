use std::time::Duration;

use super::*;

fn fixtures() -> GameFiles {
    GameFiles::directory(&crate::workspace_dir().join("tests/fixtures")).unwrap()
}

fn decode_frames(files: &GameFiles, path: &str) -> (Vec<Frame>, f64) {
    let (sender, receiver) = sync_channel(QUEUE_AHEAD);
    let files = files.clone();
    let path = path.to_owned();
    let thread = std::thread::spawn(move || decode(&files, &path, false, &sender));
    let frames: Vec<_> = receiver
        .into_iter()
        .map(|message| match message {
            Message::Frame(frame) => frame,
            Message::End(_) => unreachable!(),
        })
        .collect();
    let (count, duration) = thread.join().unwrap().unwrap();
    assert_eq!(usize::try_from(count).unwrap(), frames.len());
    (frames, duration)
}

#[test]
fn decodes_webm_key_and_inter_frames() {
    let files = fixtures();
    for codec in [
        "vp9",
        #[cfg(feature = "debug-formats")]
        "vp8",
    ] {
        let path = format!("{codec}-vorbis.webm");
        assert_eq!(probe_size(&files, &path).unwrap(), (64, 48));
        let (frames, duration) = decode_frames(&files, &path);
        assert_eq!(frames.len(), 6, "{codec}");
        assert!((0.6..0.7).contains(&duration), "{codec}: {duration}");
        let reference =
            image::load_from_memory(&files.read(&format!("{codec}-reference.png")).unwrap())
                .unwrap()
                .into_rgba8();
        for (index, frame) in frames.iter().enumerate() {
            assert_eq!(frame.image.dimensions(), (64, 48));
            if index > 0 {
                assert!((frame.pts - frames[index - 1].pts - 0.1).abs() < 0.001);
            }
            let mut error = 0u64;
            let mut max_error = 0u8;
            for (x, y, pixel) in frame.image.enumerate_pixels() {
                let expected = reference.get_pixel(x + u32::try_from(index).unwrap() * 64, y);
                for channel in 0..3 {
                    let difference = pixel[channel].abs_diff(expected[channel]);
                    error += u64::from(difference);
                    max_error = max_error.max(difference);
                }
                assert_eq!(pixel[3], 255);
            }
            // Integer YUV conversion may round differently from FFmpeg's conversion.
            assert!(
                error < 64 * 48 * 3 * 3,
                "{codec} frame {index}: total error {error}, max {max_error}"
            );
            assert!(
                max_error <= 8,
                "{codec} frame {index}: max error {max_error}"
            );
        }
        assert_ne!(
            frames[0].image, frames[5].image,
            "{codec} must decode motion"
        );
    }
}

#[test]
fn loops_video_and_stops_when_dropped() {
    check_video_loop("vp9-vorbis.webm");
    #[cfg(feature = "debug-formats")]
    for path in ["vp8-vorbis.webm", "h264-main.mp4"] {
        check_video_loop(path);
    }
}

fn check_video_loop(path: &'static str) {
    let files = fixtures();
    let count = if crate::media::is_webm(&files, path).unwrap() {
        6
    } else {
        8
    };
    let (sender, receiver) = sync_channel(QUEUE_AHEAD);
    let thread = std::thread::spawn(move || decode(&files, path, true, &sender));
    let frames: Vec<_> = (0..count * 2)
        .map(|_| {
            let Message::Frame(frame) = receiver.recv_timeout(Duration::from_secs(5)).unwrap()
            else {
                panic!("loop ended");
            };
            frame
        })
        .collect();
    assert!(frames[count].pts > frames[count - 1].pts);
    assert_eq!(frames[0].image, frames[count].image);
    assert_eq!(frames[count - 1].image, frames[count * 2 - 1].image);
    drop(receiver);
    assert!(thread.join().unwrap().is_ok());
}

#[test]
#[cfg(feature = "debug-formats")]
fn mp4_still_decodes() {
    let files = GameFiles::directory(&crate::workspace_dir().join("game")).unwrap();
    let size = probe_size(&files, "movies/intro.mp4").unwrap();
    let (frames, duration) = decode_frames(&files, "movies/intro.mp4");
    assert_eq!(frames.len(), 72);
    assert!((duration - 3.0).abs() < 0.001);
    assert!(frames.iter().all(|frame| frame.image.dimensions() == size));
    assert!(frames.windows(2).all(|pair| pair[0].pts < pair[1].pts));
}

#[test]
#[cfg(feature = "debug-formats")]
fn decodes_h264_profiles_cropping_and_reordered_frames() {
    let files = fixtures();
    for profile in ["baseline", "main", "high"] {
        let path = format!("h264-{profile}.mp4");
        assert_eq!(probe_size(&files, &path).unwrap(), (66, 50));
        let (frames, duration) = decode_frames(&files, &path);
        assert_eq!(
            frames.len(),
            8,
            "{profile}: must flush the last picture and B-frames"
        );
        assert!((duration - 0.8).abs() < 0.001);
        assert!(
            frames[0].pts.abs() < 0.001,
            "{profile}: apply the MP4 edit that removes decode delay"
        );
        assert!((frames[7].pts - 0.7).abs() < 0.001);
        let reference = image::load_from_memory(
            &files
                .read(&format!("h264-{profile}-reference.png"))
                .unwrap(),
        )
        .unwrap()
        .into_rgba8();
        for (index, frame) in frames.iter().enumerate() {
            assert_eq!(frame.image.dimensions(), (66, 50));
            if index > 0 {
                assert!((frame.pts - frames[index - 1].pts - 0.1).abs() < 0.001);
            }
            let mut error = 0u64;
            let mut max_error = 0u8;
            for (x, y, pixel) in frame.image.enumerate_pixels() {
                let expected = reference.get_pixel(x + u32::try_from(index).unwrap() * 66, y);
                for channel in 0..3 {
                    let difference = pixel[channel].abs_diff(expected[channel]);
                    error += u64::from(difference);
                    max_error = max_error.max(difference);
                }
                assert_eq!(pixel[3], 255);
            }
            assert!(
                error < 66 * 50 * 3 * 3,
                "{profile} frame {index}: total error {error}, max {max_error}"
            );
            assert!(
                max_error <= 8,
                "{profile} frame {index}: max error {max_error}"
            );
        }
        assert_ne!(
            frames[0].image, frames[7].image,
            "{profile} must decode motion"
        );
    }
}

#[test]
fn end_event_waits_for_last_frame_duration_and_fires_once() {
    let now = Instant::now();
    let (sender, receiver) = sync_channel(QUEUE_AHEAD);
    sender
        .send(Message::Frame(Frame {
            pts: 0.0,
            image: Arc::new(image::RgbaImage::new(1, 1)),
        }))
        .unwrap();
    sender.send(Message::End(0.1)).unwrap();
    let mut player = VideoPlayer {
        src: "test.webm".into(),
        looping: false,
        start: now,
        size: Some((1, 1)),
        receiver,
        state: RefCell::new(State {
            queue: VecDeque::new(),
            current: None,
            decoding_done: false,
            end_pts: 0.0,
            ended: false,
            end_reported: false,
        }),
    };
    assert!(player.frame(now).is_some());
    assert!(!player.ended());
    assert!(!player.take_ended());
    player.frame(now + Duration::from_millis(100));
    assert!(player.ended());
    assert!(player.take_ended());
    assert!(!player.take_ended());
}

#[test]
fn missing_and_invalid_webm_return_errors() {
    let files = fixtures();
    assert!(probe_size(&files, "missing.webm").is_err());
    assert!(webm::probe_size(&files, "vp8-reference.png").is_err());
    assert!(crate::media::is_webm(&files, "vp9-vorbis.webm").unwrap());
}

#[cfg(not(feature = "debug-formats"))]
#[test]
fn rejects_unconverted_mp4_and_vp8_video() {
    let files = fixtures();
    assert!(probe_size(&files, "h264-main.mp4").is_err());
    assert!(probe_size(&files, "vp8-vorbis.webm").is_err());
}
