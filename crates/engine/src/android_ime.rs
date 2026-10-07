//! `GameActivity` text state is not forwarded as IME events by Winit 0.30.

use winit::platform::android::activity::AndroidApp;
use winit::platform::android::activity::input::{
    ImeOptions, InputType, TextInputAction, TextInputState, TextSpan,
};

use crate::engine::Engine;

#[derive(Default)]
pub struct AndroidIme {
    focused: Option<(String, String)>,
    native_text: String,
}

impl AndroidIme {
    pub fn sync(&mut self, app: &AndroidApp, engine: &mut Engine) {
        let Some((id, value)) = engine.ui.focused_input_text() else {
            if self.focused.take().is_some() {
                app.hide_soft_input(false);
            }
            return;
        };
        let mut focused = (id.to_owned(), value.to_owned());
        let changed_field = self.focused.as_ref().is_none_or(|old| old.0 != focused.0);
        let state = app.text_input_state();
        if self.focused.as_ref() == Some(&focused) {
            if state.text == self.native_text {
                return;
            }
            self.native_text.clone_from(&state.text);
            if state.text == focused.1 {
                return;
            }
            engine.replace_text_input(&state.text);
            let Some((id, value)) = engine.ui.focused_input_text() else {
                self.focused = None;
                app.hide_soft_input(false);
                return;
            };
            focused = (id.to_owned(), value.to_owned());
            if focused.1 == state.text {
                // Preserve the IME's selection and composition while editing.
                self.focused = Some(focused);
                return;
            }
        }
        // set_text_input_state queues a Java-thread update. Ignore the old
        // native buffer until it changes, preserving edits made before an ack.
        self.native_text = state.text;
        let cursor = focused.1.encode_utf16().count();
        app.set_text_input_state(TextInputState {
            text: focused.1.clone(),
            selection: TextSpan {
                start: cursor,
                end: cursor,
            },
            compose_region: None,
        });
        if changed_field {
            app.set_ime_editor_info(
                InputType::TYPE_CLASS_TEXT,
                TextInputAction::None,
                ImeOptions::IMG_FLAG_NO_EXTRACT_UI,
            );
            app.show_soft_input(false);
        }
        self.focused = Some(focused);
    }
}
