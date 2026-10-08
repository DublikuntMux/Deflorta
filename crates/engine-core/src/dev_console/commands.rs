#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Inspector {
    Accessibility,
    Assets,
    Stats,
}

impl Inspector {
    pub const ALL: [Self; 3] = [Self::Accessibility, Self::Assets, Self::Stats];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Accessibility => "accessibility",
            Self::Assets => "assets",
            Self::Stats => "stats",
        }
    }

    pub const fn title(self) -> &'static str {
        match self {
            Self::Accessibility => "Accessibility tree",
            Self::Assets => "Loaded assets",
            Self::Stats => "Game resource usage",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::Accessibility => {
                "Current AccessKit tree: roles, labels, values, bounds and focus."
            }
            Self::Assets => "Loaded images, GPU textures, active media, fonts and script modules.",
            Self::Stats => {
                "Process CPU/memory/I/O, redraw timings, GPU allocations and engine counts."
            }
        }
    }

    pub const fn index(self) -> usize {
        match self {
            Self::Accessibility => 0,
            Self::Assets => 1,
            Self::Stats => 2,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Help(Option<Inspector>),
    HelpUnload,
    Inspect { kind: Inspector, window: bool },
    Unload { source: String },
}

pub fn parse(source: &str) -> Option<Result<Command, String>> {
    let mut words = source.split_whitespace();
    let first = words.next()?;
    let kind = Inspector::ALL.into_iter().find(|kind| kind.name() == first);
    if first == "help" {
        return Some(match (words.next(), words.next()) {
            (None | Some("help"), None) => Ok(Command::Help(None)),
            (Some("unload"), None) => Ok(Command::HelpUnload),
            (Some(name), None) => Inspector::ALL
                .into_iter()
                .find(|kind| kind.name() == name)
                .map(|kind| Command::Help(Some(kind)))
                .ok_or_else(|| {
                    format!("Unknown command '{name}'. Type help for the command list.")
                }),
            _ => Err("Usage: help [command]".into()),
        });
    }
    if first == "unload" {
        return Some(parse_unload(source.trim_start()[first.len()..].trim()));
    }
    let kind = kind?;
    if kind == Inspector::Assets && words.clone().next() == Some("unload") {
        let arguments = source.trim_start()[first.len()..].trim_start();
        return Some(parse_unload(arguments["unload".len()..].trim()));
    }
    Some(match (words.next(), words.next()) {
        (None, None) => Ok(Command::Inspect {
            kind,
            window: false,
        }),
        (Some("--window"), None) => Ok(Command::Inspect { kind, window: true }),
        _ => Err(format!("Usage: {} [--window]", kind.name())),
    })
}

fn parse_unload(arguments: &str) -> Result<Command, String> {
    let source = if arguments.starts_with(['\'', '"']) {
        let quote = arguments.chars().next().unwrap();
        arguments
            .strip_prefix(quote)
            .and_then(|source| source.strip_suffix(quote))
            .ok_or_else(|| "Usage: unload <asset id> (close the quoted asset ID)".to_owned())?
    } else {
        arguments
    };
    if source.is_empty() {
        return Err("Usage: unload <asset id> (or assets unload <asset id>)".into());
    }
    Ok(Command::Unload {
        source: source.to_owned(),
    })
}

pub const UNLOAD_HELP: &str = "unload <asset id> (or assets unload <asset id>) — Force release an image, GPU texture or audio/video playback.\nUse the source/ID printed by assets; IDs may include spaces or ?query suffixes.\nImages referenced by the UI reload on the next draw. Audio/video playback stops. Fonts and JavaScript modules stay loaded for the game session.";

pub fn help(kind: Option<Inspector>) -> String {
    let mut text = if kind.is_none() {
        "help [command] — List built-in commands or describe one command.\n".to_owned()
    } else {
        String::new()
    };
    for command in Inspector::ALL
        .into_iter()
        .filter(|command| kind.is_none_or(|kind| kind == *command))
    {
        use std::fmt::Write as _;
        writeln!(
            text,
            "{} [--window] — {}",
            command.name(),
            command.description()
        )
        .unwrap();
    }
    if kind.is_none() || kind == Some(Inspector::Assets) {
        text.push_str(UNLOAD_HELP);
        text.push('\n');
    }
    text.push_str("Without --window: print a snapshot. With --window: open a floating egui inspector, refreshed every 500 ms.\nOther input is evaluated as JavaScript in the live game realm.");
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unload_accepts_source_ids_and_validates_missing_or_unclosed_arguments() {
        for input in [
            "unload images/title screen.png?2",
            " assets unload images/title screen.png?2 ",
            "unload \"images/title screen.png?2\"",
            "assets unload 'images/title screen.png?2'",
        ] {
            assert_eq!(
                parse(input),
                Some(Ok(Command::Unload {
                    source: "images/title screen.png?2".into()
                }))
            );
        }
        for input in [
            "unload",
            "assets unload",
            "unload \"\"",
            "unload \"unclosed",
            "assets unload 'unclosed",
        ] {
            assert!(parse(input).unwrap().is_err());
        }
        assert_eq!(parse("help unload"), Some(Ok(Command::HelpUnload)));
        assert!(help(Some(Inspector::Assets)).contains("assets unload <asset id>"));
        assert_eq!(parse("unload('image.png')"), None);
        assert_eq!(parse("assets.unload('image.png')"), None);
    }

    #[test]
    fn builtins_validate_options_and_preserve_javascript() {
        assert_eq!(parse("help"), Some(Ok(Command::Help(None))));
        assert_eq!(
            parse("help assets"),
            Some(Ok(Command::Help(Some(Inspector::Assets))))
        );
        for kind in Inspector::ALL {
            assert_eq!(
                parse(kind.name()),
                Some(Ok(Command::Inspect {
                    kind,
                    window: false
                }))
            );
            assert_eq!(
                parse(&format!("{} --window", kind.name())),
                Some(Ok(Command::Inspect { kind, window: true }))
            );
            assert!(
                parse(&format!("{} --window --window", kind.name()))
                    .unwrap()
                    .is_err()
            );
            assert!(help(None).contains(kind.description()));
        }
        assert_eq!(parse("help()"), None);
        assert_eq!(parse("assets.length"), None);
        assert_eq!(parse("const stats = 3; stats"), None);
        assert_eq!(parse("deflorta.store"), None);
        assert!(parse("stats --wat").unwrap().is_err());
        assert!(parse("help unknown").unwrap().is_err());
    }
}
