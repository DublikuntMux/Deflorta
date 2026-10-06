use std::time::Duration;

use super::*;

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn decode_frames(path: &Path) -> (Vec<Frame>, f64) {
    let (sender, receiver) = sync_channel(QUEUE_AHEAD);
    let path = path.to_owned();
    let thread = std::thread::spawn(move || decode(&path, false, &sender));
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
    for codec in ["vp8", "vp9"] {
        let path = fixture(&format!("{codec}-vorbis.webm"));
        assert_eq!(probe_size(&path).unwrap(), (64, 48));
        let (frames, duration) = decode_frames(&path);
        assert_eq!(frames.len(), 6, "{codec}");
        assert!((0.6..0.7).contains(&duration), "{codec}: {duration}");
        let reference = image::open(fixture(&format!("{codec}-reference.png")))
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
fn loops_webm_and_stops_when_dropped() {
    for codec in ["vp8", "vp9"] {
        let path = fixture(&format!("{codec}-vorbis.webm"));
        let (sender, receiver) = sync_channel(QUEUE_AHEAD);
        let thread = std::thread::spawn(move || decode(&path, true, &sender));
        let frames: Vec<_> = (0..12)
            .map(|_| {
                let Message::Frame(frame) = receiver.recv_timeout(Duration::from_secs(5)).unwrap()
                else {
                    panic!("loop ended");
                };
                frame
            })
            .collect();
        assert!(frames[6].pts > frames[5].pts);
        assert_eq!(frames[0].image, frames[6].image);
        assert_eq!(frames[5].image, frames[11].image);
        drop(receiver);
        assert!(thread.join().unwrap().is_ok());
    }
}

#[test]
fn mp4_still_decodes() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("game/movies/intro.mp4");
    let size = probe_size(&path).unwrap();
    let (frames, duration) = decode_frames(&path);
    assert!(!frames.is_empty());
    assert!(duration > 0.0);
    assert!(frames.iter().all(|frame| frame.image.dimensions() == size));
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
    assert!(probe_size(&fixture("missing.webm")).is_err());
    assert!(webm::probe_size(&fixture("vp8-reference.png")).is_err());
    assert!(is_webm(Path::new("MOVIE.WEBM")));
}
