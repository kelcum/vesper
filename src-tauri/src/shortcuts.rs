use crate::state::{lock, AppState};
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

pub fn parse(text: &str) -> Result<Shortcut, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("no keys".into());
    }
    let shortcut = trimmed.parse::<Shortcut>().map_err(|e| e.to_string())?;
    if shortcut.mods.is_empty() {
        return Err("a hotkey needs Ctrl, Alt, Shift or the Windows key".into());
    }
    Ok(shortcut)
}

pub fn sync(app: &AppHandle) {
    let shortcuts = app.global_shortcut();
    if let Err(e) = shortcuts.unregister_all() {
        log::warn!("Couldn't clear hotkeys: {e}");
    }
    let Some(text) = app.state::<AppState>().settings().shaders.hotkey else { return };
    let shortcut = match parse(&text) {
        Ok(s) => s,
        Err(e) => {
            log::warn!("The shaders hotkey {text} isn't valid: {e}");
            return;
        }
    };
    let result = shortcuts.on_shortcut(shortcut, |app, _, event| {
        if event.state() == ShortcutState::Pressed {
            toggle_shaders(app);
        }
    });
    match result {
        Ok(()) => log::info!("Shaders hotkey: {text}"),
        Err(e) => {
            log::warn!("Couldn't register the shaders hotkey {text}: {e}");
            lock(&app.state::<AppState>().notices)
                .push(format!("The shaders hotkey {text} couldn't be set up. Another program may already use it."));
        }
    }
}

fn toggle_shaders(app: &AppHandle) {
    let state = app.state::<AppState>();
    let on = !state.settings().shaders.enabled;
    let patch = serde_json::json!({ "shaders": { "enabled": on } });
    if let Err(e) = crate::commands::settings_update(app.clone(), state, patch) {
        log::warn!("Couldn't switch shaders from the hotkey: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotkeys_need_a_modifier_and_a_key() {
        assert!(parse("Ctrl+Alt+S").is_ok());
        assert!(parse(" CmdOrCtrl+Shift+F8 ").is_ok());
        assert!(parse("S").is_err());
        assert!(parse("").is_err());
        assert!(parse("Ctrl+").is_err());
        assert!(parse("Ctrl+Nope").is_err());
    }
}
