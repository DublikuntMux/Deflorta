use super::*;

fn style() -> TextStyle {
    TextStyle {
        font_size: 24.0,
        line_height: 1.3,
        family: "Noto Sans".into(),
        weight: 400,
        italic: false,
        align: None,
    }
}

#[test]
fn cached_text_reuses_content_and_tracks_physical_wrap_width() {
    let game = Path::new(env!("CARGO_MANIFEST_DIR")).join("game");
    let mut text = TextSystem::new(&game);
    let spans = vec![SpanDesc {
        text: "One two three four five six seven eight nine ten".into(),
        ..Default::default()
    }];
    let style = style();
    let first = text.prepare(
        "text",
        &spans,
        &style,
        1.0,
        Some(160.0),
        usize::MAX,
        1.0,
        None,
    );
    let content = text
        .entry("text")
        .unwrap()
        .shaped
        .as_ref()
        .unwrap()
        .spans
        .as_ptr();
    assert_eq!(
        text.prepare(
            "text",
            &spans,
            &style,
            1.0,
            Some(160.0),
            usize::MAX,
            0.5,
            None
        ),
        first
    );
    assert_eq!(
        text.entry("text")
            .unwrap()
            .shaped
            .as_ref()
            .unwrap()
            .spans
            .as_ptr(),
        content
    );
    let scaled = text.prepare(
        "text",
        &spans,
        &style,
        2.0,
        Some(160.0),
        usize::MAX,
        1.0,
        None,
    );
    assert_eq!(text.entry("text").unwrap().buffer.size().0, Some(320.0));
    let mut fresh = TextSystem::new(&game);
    assert_eq!(
        scaled,
        fresh.prepare(
            "text",
            &spans,
            &style,
            2.0,
            Some(160.0),
            usize::MAX,
            1.0,
            None
        )
    );
}

#[test]
fn unkeyed_nodes_keep_their_text_shadow_and_ruby_caches() {
    let game = Path::new(env!("CARGO_MANIFEST_DIR")).join("game");
    let mut text = TextSystem::new(&game);
    let spans = [SpanDesc {
        text: "hello".into(),
        ruby: Some("hi".into()),
        ..Default::default()
    }];
    for id in ["/root/#0", "/root/#0#shadow", "/root/#1"] {
        text.prepare(id, &spans, &style(), 1.0, None, usize::MAX, 1.0, None);
    }
    text.retain(|id| id == "/root/#0");
    for id in [
        "/root/#0",
        "/root/#0#ruby0",
        "/root/#0#shadow",
        "/root/#0#shadow#ruby0",
    ] {
        assert!(text.entry(id).is_some(), "missing {id}");
    }
    assert!(text.entry("/root/#1").is_none());
    assert!(text.entry("/root/#1#ruby0").is_none());
}
