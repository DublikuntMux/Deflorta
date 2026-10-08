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
    let game = crate::GameFiles::open(&crate::workspace_dir().join("game")).unwrap();
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
        TextOptions {
            width: Some(160.0),
            ..TextOptions::default()
        },
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
            TextOptions {
                width: Some(160.0),
                alpha: 0.5,
                ..TextOptions::default()
            }
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
        TextOptions {
            scale: 2.0,
            width: Some(160.0),
            ..TextOptions::default()
        },
    );
    assert_eq!(text.entry("text").unwrap().buffer.size().0, Some(320.0));
    let mut fresh = TextSystem::new(&game);
    assert_eq!(
        scaled,
        fresh.prepare(
            "text",
            &spans,
            &style,
            TextOptions {
                scale: 2.0,
                width: Some(160.0),
                ..TextOptions::default()
            }
        )
    );
}

#[test]
fn unkeyed_nodes_keep_their_text_shadow_and_ruby_caches() {
    let game = crate::GameFiles::open(&crate::workspace_dir().join("game")).unwrap();
    let mut text = TextSystem::new(&game);
    let spans = [SpanDesc {
        text: "hello".into(),
        ruby: Some("hi".into()),
        ..Default::default()
    }];
    for id in ["/root/#0", "/root/#0#shadow", "/root/#1"] {
        text.prepare(id, &spans, &style(), TextOptions::default());
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
