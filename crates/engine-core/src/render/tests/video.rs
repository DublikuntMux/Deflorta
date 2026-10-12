use super::super::*;
use deflorta_assets::video::{ColorMatrix, ColorRange, Plane, VideoFrame};

pub(super) fn invalid_frame() -> VideoFrame {
    VideoFrame {
        planes: std::array::from_fn(|_| Plane {
            data: vec![],
            stride: 0,
            width: 0,
            height: 0,
        }),
        alpha: None,
        matrix: ColorMatrix::Bt601,
        range: ColorRange::Limited,
        subsampling: (true, true),
    }
}

fn sample(plane: &Plane, x: u32, y: u32) -> u8 {
    plane.data[usize::try_from(u64::from(y) * u64::from(plane.stride) + u64::from(x)).unwrap()]
}

fn assert_conversion(output: &image::RgbaImage, frame: &VideoFrame) {
    let rows = frame.rgb_transform();
    for (x, y, pixel) in output.enumerate_pixels() {
        let cx = x >> u32::from(frame.subsampling.0);
        let cy = y >> u32::from(frame.subsampling.1);
        let samples = [
            f32::from(sample(&frame.planes[0], x, y)) / 255.0,
            f32::from(sample(&frame.planes[1], cx, cy)) / 255.0,
            f32::from(sample(&frame.planes[2], cx, cy)) / 255.0,
            1.0,
        ];
        for (channel, row) in rows.iter().enumerate() {
            let expected: f32 = row.iter().zip(samples).map(|(a, b)| a * b).sum();
            let expected =
                num_traits::ToPrimitive::to_u8(&(expected.clamp(0.0, 1.0) * 255.0).round())
                    .unwrap();
            assert!(
                pixel[channel].abs_diff(expected) <= 1,
                "{:?}/{:?}/{:?} at {x},{y}: {pixel:?}",
                frame.matrix,
                frame.range,
                frame.subsampling
            );
        }
        assert_eq!(
            pixel[3],
            frame.alpha.as_ref().map_or(255, |a| sample(a, x, y))
        );
    }
}

#[test]
#[ignore = "Requires a native GPU adapter"]
fn planar_conversion_preserves_color_alpha_strides_and_odd_sizes() {
    let files = crate::GameFiles::open(&crate::workspace_dir().join("game")).unwrap();
    let mut assets = Assets::new(files);
    let mut renderer = pollster::block_on(Renderer::offscreen(5, 3)).unwrap();
    let plane = |width: u32, height: u32, base: u8| {
        let stride = width + 3;
        let data: Vec<_> = (0..height)
            .flat_map(|y| {
                (0..stride).map(move |x| {
                    if x < width {
                        base + u8::try_from(y * width + x).unwrap()
                    } else {
                        255
                    }
                })
            })
            .collect();
        Plane::copy(&data, stride, width, height).unwrap()
    };
    for subsampling in [(true, true), (true, false), (false, false)] {
        for matrix in [
            ColorMatrix::Bt601,
            ColorMatrix::Bt709,
            ColorMatrix::Bt2020,
            ColorMatrix::Smpte240,
            ColorMatrix::Identity,
        ] {
            for range in [ColorRange::Limited, ColorRange::Full] {
                for alpha in [false, true] {
                    let cw = if subsampling.0 { 3 } else { 5 };
                    let ch = if subsampling.1 { 2 } else { 3 };
                    let frame = Arc::new(VideoFrame {
                        planes: [plane(5, 3, 100), plane(cw, ch, 90), plane(cw, ch, 170)],
                        alpha: alpha.then(|| {
                            Plane::copy(
                                &[
                                    0, 16, 128, 235, 255, 99, 99, 99, 1, 2, 3, 4, 5, 99, 99, 99,
                                    255, 235, 128, 16, 0,
                                ],
                                8,
                                5,
                                3,
                            )
                            .unwrap()
                        }),
                        matrix,
                        range,
                        subsampling,
                    });
                    let serial = renderer
                        .textures
                        .get("video:test")
                        .and_then(|c| match &c.texture {
                            Some(Resident::Own(t)) => Some(t.serial + 1),
                            _ => None,
                        })
                        .unwrap_or(1);
                    let image = ImageRef {
                        src: "video:test".into(),
                        uv: [0.0, 0.0, 1.0, 1.0],
                        frame: Some((serial, frame.clone())),
                    };
                    renderer
                        .ensure_texture(&image, &mut assets, Instant::now())
                        .unwrap();
                    let Some(Resident::Own(texture)) = &renderer.textures["video:test"].texture
                    else {
                        panic!("missing video texture")
                    };
                    let output = renderer.read_texture(&texture.raw).unwrap();
                    assert_conversion(&output, &frame);
                }
            }
        }
    }
}

#[test]
#[ignore = "Requires a native GPU adapter"]
fn video_is_converted_for_offscreen_and_thumbnail_capture() {
    let files = crate::GameFiles::open(&crate::workspace_dir().join("game")).unwrap();
    let mut assets = Assets::new(files.clone());
    let mut ui = Ui::new(deflorta_ui::ui::text::TextSystem::new(&files));
    ui.set_config(5.0, 3.0, "Noto Sans");
    ui.set_surface_size(5.0, 3.0);
    let mut renderer = pollster::block_on(Renderer::offscreen(5, 3)).unwrap();
    let mut frame = VideoFrame::copy(
        (5, 3),
        [(&[120; 15], 5), (&[180; 15], 5), (&[60; 15], 5)],
        (false, false),
        ColorRange::Full,
        ColorMatrix::Identity,
    )
    .unwrap();
    frame.alpha = Some(
        Plane::copy(
            &[
                0, 16, 128, 235, 255, 0, 16, 128, 235, 255, 0, 16, 128, 235, 255,
            ],
            5,
            5,
            3,
        )
        .unwrap(),
    );
    let rect = Rect {
        x: 0.0,
        y: 0.0,
        w: 5.0,
        h: 3.0,
    };
    let items = vec![DrawItem::Quad(Quad {
        rect,
        rotation: 0.0,
        color: Color::WHITE,
        radii: [0.0; 4],
        border_width: 0.0,
        border_color: Color::TRANSPARENT,
        image: Some(ImageRef {
            src: "video:capture".into(),
            uv: [0.0, 0.0, 1.0, 1.0],
            frame: Some((1, Arc::new(frame))),
        }),
        clip: rect,
        mask: None,
    })];
    let background = Color([0.0, 0.0, 0.0, 1.0]);
    let thumbnail = renderer
        .render_to_image(items.clone(), &mut ui, &mut assets, background)
        .unwrap();
    let Some(Resident::Own(texture)) = &renderer.textures["video:capture"].texture else {
        panic!("missing video texture")
    };
    let original = texture.raw.clone();
    renderer
        .render(
            items,
            &mut ui,
            &mut assets,
            background,
            #[cfg(feature = "dev-console")]
            None,
        )
        .unwrap();
    assert_eq!(renderer.capture().unwrap(), thumbnail);
    let Some(Resident::Own(texture)) = &renderer.textures["video:capture"].texture else {
        panic!("missing video texture")
    };
    assert_eq!(
        texture.raw, original,
        "capture reuses the converted video texture"
    );
    for (x, _, pixel) in thumbnail.enumerate_pixels() {
        let alpha = [0_u16, 16, 128, 235, 255][usize::try_from(x).unwrap()];
        for (channel, color) in [60_u16, 120, 180].into_iter().enumerate() {
            let expected = u8::try_from((color * alpha + 127) / 255).unwrap();
            assert!(
                pixel[channel].abs_diff(expected) <= 1,
                "alpha at {x}: {pixel:?}"
            );
        }
        assert_eq!(pixel[3], 255);
    }
}
