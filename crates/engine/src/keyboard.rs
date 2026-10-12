//! Stable script key names. Never derive the public contract from Debug output.

use winit::keyboard::{Key, NamedKey};

pub(super) fn key_name(key: &Key) -> Option<String> {
    match key {
        #[cfg(target_os = "android")]
        Key::Named(NamedKey::BrowserBack) => Some("Escape".into()),
        Key::Named(named) => named_key(*named).map(str::to_owned),
        Key::Character(text) => Some(text.to_string()),
        _ => None,
    }
}

const NAMED_KEYS: &[(NamedKey, &str)] = &[
    (NamedKey::Space, " "),
    (NamedKey::Alt, "Alt"),
    (NamedKey::AltGraph, "AltGraph"),
    (NamedKey::CapsLock, "CapsLock"),
    (NamedKey::Control, "Control"),
    (NamedKey::Fn, "Fn"),
    (NamedKey::FnLock, "FnLock"),
    (NamedKey::NumLock, "NumLock"),
    (NamedKey::ScrollLock, "ScrollLock"),
    (NamedKey::Shift, "Shift"),
    (NamedKey::Symbol, "Symbol"),
    (NamedKey::SymbolLock, "SymbolLock"),
    (NamedKey::Meta, "Meta"),
    (NamedKey::Hyper, "Hyper"),
    (NamedKey::Super, "Super"),
    (NamedKey::Enter, "Enter"),
    (NamedKey::Tab, "Tab"),
    (NamedKey::ArrowDown, "ArrowDown"),
    (NamedKey::ArrowLeft, "ArrowLeft"),
    (NamedKey::ArrowRight, "ArrowRight"),
    (NamedKey::ArrowUp, "ArrowUp"),
    (NamedKey::End, "End"),
    (NamedKey::Home, "Home"),
    (NamedKey::PageDown, "PageDown"),
    (NamedKey::PageUp, "PageUp"),
    (NamedKey::Backspace, "Backspace"),
    (NamedKey::Clear, "Clear"),
    (NamedKey::Copy, "Copy"),
    (NamedKey::CrSel, "CrSel"),
    (NamedKey::Cut, "Cut"),
    (NamedKey::Delete, "Delete"),
    (NamedKey::EraseEof, "EraseEof"),
    (NamedKey::ExSel, "ExSel"),
    (NamedKey::Insert, "Insert"),
    (NamedKey::Paste, "Paste"),
    (NamedKey::Redo, "Redo"),
    (NamedKey::Undo, "Undo"),
    (NamedKey::Accept, "Accept"),
    (NamedKey::Again, "Again"),
    (NamedKey::Attn, "Attn"),
    (NamedKey::Cancel, "Cancel"),
    (NamedKey::ContextMenu, "ContextMenu"),
    (NamedKey::Escape, "Escape"),
    (NamedKey::Execute, "Execute"),
    (NamedKey::Find, "Find"),
    (NamedKey::Help, "Help"),
    (NamedKey::Pause, "Pause"),
    (NamedKey::Play, "Play"),
    (NamedKey::Props, "Props"),
    (NamedKey::Select, "Select"),
    (NamedKey::ZoomIn, "ZoomIn"),
    (NamedKey::ZoomOut, "ZoomOut"),
    (NamedKey::PrintScreen, "PrintScreen"),
    (NamedKey::Compose, "Compose"),
    (NamedKey::Convert, "Convert"),
    (NamedKey::NonConvert, "NonConvert"),
    (NamedKey::ModeChange, "ModeChange"),
    (NamedKey::Process, "Process"),
    (NamedKey::MediaPlay, "MediaPlay"),
    (NamedKey::MediaPause, "MediaPause"),
    (NamedKey::MediaPlayPause, "MediaPlayPause"),
    (NamedKey::MediaStop, "MediaStop"),
    (NamedKey::MediaRecord, "MediaRecord"),
    (NamedKey::MediaTrackNext, "MediaTrackNext"),
    (NamedKey::MediaTrackPrevious, "MediaTrackPrevious"),
    (NamedKey::MediaFastForward, "MediaFastForward"),
    (NamedKey::MediaRewind, "MediaRewind"),
    (NamedKey::MediaSkipBackward, "MediaSkipBackward"),
    (NamedKey::MediaSkipForward, "MediaSkipForward"),
    (NamedKey::AudioVolumeDown, "AudioVolumeDown"),
    (NamedKey::AudioVolumeUp, "AudioVolumeUp"),
    (NamedKey::AudioVolumeMute, "AudioVolumeMute"),
    (NamedKey::MicrophoneToggle, "MicrophoneToggle"),
    (NamedKey::MicrophoneVolumeMute, "MicrophoneVolumeMute"),
    (NamedKey::Eject, "Eject"),
    (NamedKey::BrowserBack, "BrowserBack"),
    (NamedKey::BrowserForward, "BrowserForward"),
    (NamedKey::BrowserHome, "BrowserHome"),
    (NamedKey::BrowserRefresh, "BrowserRefresh"),
    (NamedKey::BrowserSearch, "BrowserSearch"),
    (NamedKey::BrowserStop, "BrowserStop"),
    (NamedKey::BrowserFavorites, "BrowserFavorites"),
    (NamedKey::GoBack, "GoBack"),
    (NamedKey::GoHome, "GoHome"),
    (NamedKey::AppSwitch, "AppSwitch"),
    (NamedKey::Call, "Call"),
    (NamedKey::EndCall, "EndCall"),
    (NamedKey::Camera, "Camera"),
    (NamedKey::CameraFocus, "CameraFocus"),
    (NamedKey::HeadsetHook, "HeadsetHook"),
    (NamedKey::F1, "F1"),
    (NamedKey::F2, "F2"),
    (NamedKey::F3, "F3"),
    (NamedKey::F4, "F4"),
    (NamedKey::F5, "F5"),
    (NamedKey::F6, "F6"),
    (NamedKey::F7, "F7"),
    (NamedKey::F8, "F8"),
    (NamedKey::F9, "F9"),
    (NamedKey::F10, "F10"),
    (NamedKey::F11, "F11"),
    (NamedKey::F12, "F12"),
    (NamedKey::F13, "F13"),
    (NamedKey::F14, "F14"),
    (NamedKey::F15, "F15"),
    (NamedKey::F16, "F16"),
    (NamedKey::F17, "F17"),
    (NamedKey::F18, "F18"),
    (NamedKey::F19, "F19"),
    (NamedKey::F20, "F20"),
    (NamedKey::F21, "F21"),
    (NamedKey::F22, "F22"),
    (NamedKey::F23, "F23"),
    (NamedKey::F24, "F24"),
    (NamedKey::F25, "F25"),
    (NamedKey::F26, "F26"),
    (NamedKey::F27, "F27"),
    (NamedKey::F28, "F28"),
    (NamedKey::F29, "F29"),
    (NamedKey::F30, "F30"),
    (NamedKey::F31, "F31"),
    (NamedKey::F32, "F32"),
    (NamedKey::F33, "F33"),
    (NamedKey::F34, "F34"),
    (NamedKey::F35, "F35"),
];

fn named_key(key: NamedKey) -> Option<&'static str> {
    NAMED_KEYS
        .iter()
        .find_map(|&(named, name)| (named == key).then_some(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_preserve_runtime_bindings_and_ignore_unknown_keys() {
        for (key, expected) in [
            (NamedKey::Enter, "Enter"),
            (NamedKey::Space, " "),
            (NamedKey::Escape, "Escape"),
            (NamedKey::Control, "Control"),
            (NamedKey::PageUp, "PageUp"),
            (NamedKey::Backspace, "Backspace"),
            (NamedKey::ArrowUp, "ArrowUp"),
            (NamedKey::ArrowDown, "ArrowDown"),
            (NamedKey::F6, "F6"),
            (NamedKey::F11, "F11"),
            (NamedKey::MediaPlayPause, "MediaPlayPause"),
        ] {
            assert_eq!(key_name(&Key::Named(key)).as_deref(), Some(expected));
        }
        assert_eq!(key_name(&Key::Character("h".into())).as_deref(), Some("h"));
        assert_eq!(key_name(&Key::Named(NamedKey::TV)), None);
        assert_eq!(key_name(&Key::Dead(None)), None);
    }
}
