//! The Avatar page: Roblox's own website, docked inside Vesper's window, and
//! Vesper's avatar tools that work through it.
//!
//! Signing in happens on Roblox's real login page. The session stays in a
//! separate browser profile that belongs to that page; Vesper's code never
//! reads the password or the session cookie. When Vesper's tools change the
//! avatar, they ask the Roblox page to make the request itself (as RoValra
//! and RoSeal do from inside the browser), so the cookie never leaves it.
//! Only Roblox's avatar, catalog, inventory, thumbnail and user APIs can be
//! called this way, and Roblox applies its own rules to every change.

use crate::error::{Problem, ProblemAction, Result};
use crate::state::AppState;
use serde::Deserialize;
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

pub const LABEL: &str = "roblox";
const EDITOR_URL: &str = "https://www.roblox.com/my/avatar";
/// A light page on Roblox's site that keeps the session usable for requests.
const BRIDGE_URL: &str = "https://www.roblox.com/robots.txt";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(25);

/// Roblox's APIs Vesper's avatar tools may call.
const API_HOSTS: &[&str] = &[
    "avatar.roblox.com",
    "users.roblox.com",
    "thumbnails.roblox.com",
    "inventory.roblox.com",
    "catalog.roblox.com",
    "economy.roblox.com",
    "auth.roblox.com",
];

/// Pages the docked browser may show; anything else opens in the normal browser.
fn allowed_page(url: &url::Url) -> bool {
    let host = url.host_str().unwrap_or_default();
    url.scheme() == "https"
        && (host == "roblox.com"
            || host.ends_with(".roblox.com")
            || host.ends_with(".rbxcdn.com")
            || host.ends_with(".arkoselabs.com")
            || host.ends_with(".funcaptcha.com")
            || host.ends_with(".hcaptcha.com"))
}

pub fn allowed_api(url: &url::Url) -> bool {
    url.scheme() == "https" && url.host_str().is_some_and(|h| API_HOSTS.contains(&h)) && url.username().is_empty()
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Bounds {
    /// Physical pixels inside Vesper's window.
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

fn problem(what: impl Into<String>) -> Problem {
    Problem::new("avatar", "The avatar editor ran into a problem")
        .what(what)
        .why("Roblox's website or its connection didn't respond as expected.")
        .fix("Try again. If it keeps happening, reload the Roblox page from the toolbar.")
        .action(ProblemAction::Retry)
}

fn window(app: &AppHandle) -> Option<tauri::WebviewWindow> {
    app.get_webview_window(LABEL)
}

/// Opens (or shows) the docked Roblox page.
#[tauri::command]
pub async fn avatar_open(app: AppHandle, bounds: Bounds, page: Option<String>, visible: bool) -> Result<()> {
    let url = match page.as_deref() {
        Some("bridge") => BRIDGE_URL,
        Some("login") => "https://www.roblox.com/login?ReturnUrl=%2Fmy%2Favatar",
        _ => EDITOR_URL,
    };
    if let Some(w) = window(&app) {
        if page.is_some() && w.url().map(|u| u.as_str() != url).unwrap_or(true) {
            let _ = w.navigate(url.parse().expect("valid url"));
        }
        if visible {
            dock(&app, &w, bounds)?;
        } else {
            hide(&w);
        }
        return Ok(());
    }
    let state = app.state::<AppState>();
    let data = state.paths.cache.join("roblox-web");
    let opener = app.clone();
    let opener2 = app.clone();
    let w = WebviewWindowBuilder::new(&app, LABEL, WebviewUrl::External(url.parse().expect("valid url")))
        .title("Roblox")
        .decorations(false)
        .resizable(false)
        .skip_taskbar(true)
        .visible(false)
        .shadow(false)
        .focused(false)
        .data_directory(data)
        .background_color(tauri::window::Color(18, 18, 22, 255))
        .on_navigation(move |u| {
            if allowed_page(u) {
                return true;
            }
            open_outside(&opener, u);
            false
        })
        .on_new_window(move |u, _| {
            if allowed_page(&u) && u.path().starts_with("/my/") {
                // Roblox pages that try to open a tab stay in the docked page.
                if let Some(w) = window(&opener2) {
                    let _ = w.navigate(u);
                }
            } else {
                open_outside(&opener2, &u);
            }
            tauri::webview::NewWindowResponse::Deny
        })
        .build()
        .map_err(|e| problem(format!("Couldn't open Roblox's page: {e}")))?;
    if visible {
        dock(&app, &w, bounds)?;
    }
    log::info!("Opened Roblox's website for the avatar editor");
    Ok(())
}

fn open_outside(app: &AppHandle, u: &url::Url) {
    use tauri_plugin_opener::OpenerExt;
    if u.scheme() == "https" {
        let _ = app.opener().open_url(u.as_str(), None::<&str>);
    }
}

/// Puts the Roblox page inside Vesper's window at `bounds` and shows it.
///
/// Showing and hiding go straight to the window: once it's docked, Tauri's own
/// show/hide would restyle it as a top-level window (and its hide is skipped,
/// as Tauri still thinks the window is hidden).
#[cfg(windows)]
fn dock(app: &AppHandle, w: &tauri::WebviewWindow, b: Bounds) -> Result<()> {
    let main = app.get_webview_window("main").ok_or_else(|| problem("Vesper's window isn't available."))?;
    let (parent, child) =
        (main.hwnd().map_err(|e| problem(e.to_string()))?, w.hwnd().map_err(|e| problem(e.to_string()))?);
    if imp::dock(parent.0 as isize, child.0 as isize, b) {
        // It was hidden: tell the page it's on screen again, or it can stay blank.
        let page: &tauri::Webview = w.as_ref();
        let _ = page.hide();
        let _ = page.show();
    }
    Ok(())
}

#[cfg(windows)]
fn hide(w: &tauri::WebviewWindow) {
    if let Ok(h) = w.hwnd() {
        imp::hide(h.0 as isize);
    }
}

#[cfg(not(windows))]
fn hide(w: &tauri::WebviewWindow) {
    let _ = w.hide();
}

/// Elsewhere the page is its own window, placed over the same spot.
#[cfg(not(windows))]
fn dock(app: &AppHandle, w: &tauri::WebviewWindow, b: Bounds) -> Result<()> {
    let main = app.get_webview_window("main").ok_or_else(|| problem("Vesper's window isn't available."))?;
    let origin = main.inner_position().map_err(|e| problem(e.to_string()))?;
    let _ = w.set_position(tauri::PhysicalPosition::new(origin.x + b.x, origin.y + b.y));
    let _ = w.set_size(tauri::PhysicalSize::new(b.width.max(1) as u32, b.height.max(1) as u32));
    let _ = w.show();
    Ok(())
}

#[tauri::command]
pub fn avatar_bounds(app: AppHandle, bounds: Bounds) -> Result<()> {
    if let Some(w) = window(&app) {
        dock(&app, &w, bounds)?;
    }
    Ok(())
}

#[tauri::command]
pub fn avatar_hide(app: AppHandle) {
    if let Some(w) = window(&app) {
        hide(&w);
    }
}

/// Closes the page (the sign-in is kept for next time).
#[tauri::command]
pub fn avatar_close(app: AppHandle) {
    if let Some(w) = window(&app) {
        let _ = w.close();
    }
}

#[tauri::command]
pub fn avatar_reload(app: AppHandle) {
    if let Some(w) = window(&app) {
        let _ = w.eval("location.reload()");
    }
}

static NEXT: AtomicU64 = AtomicU64::new(1);

/// Runs a request from inside the Roblox page, so it carries the player's
/// session without Vesper handling it. Returns `(status, body)`.
async fn page_fetch(app: &AppHandle, method: &str, url: &url::Url, body: Option<&Value>) -> Result<(u16, Value)> {
    let w = window(app).ok_or_else(|| problem("Roblox's page isn't open."))?;
    let current = w.url().map_err(|e| problem(e.to_string()))?;
    if current.host_str() != Some("www.roblox.com") {
        return Err(problem("Roblox's page is still loading. Try again in a moment."));
    }
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    let script = format!(
        r#"(async () => {{
  const v = (window.__vesper = window.__vesper || {{ r: {{}}, token: null }});
  const send = (t) => fetch({url}, {{
    method: {method},
    credentials: "include",
    headers: Object.assign({{ "Accept": "application/json", "Content-Type": "application/json" }}, t ? {{ "x-csrf-token": t }} : {{}}),
    body: {body},
  }});
  try {{
    let res = await send(v.token);
    const t = res.headers.get("x-csrf-token");
    if (res.status === 403 && t) {{ v.token = t; res = await send(t); }}
    v.r[{id}] = JSON.stringify({{ status: res.status, body: await res.text() }});
  }} catch (e) {{
    v.r[{id}] = JSON.stringify({{ status: 0, body: String(e) }});
  }}
}})()"#,
        url = serde_json::to_string(url.as_str()).expect("string"),
        method = serde_json::to_string(method).expect("string"),
        body = match body {
            Some(b) => serde_json::to_string(&b.to_string()).expect("string"),
            None => "undefined".into(),
        },
    );
    w.eval(script).map_err(|e| problem(e.to_string()))?;
    let take = format!(
        "(() => {{ const v = window.__vesper && window.__vesper.r[{id}]; if (v) delete window.__vesper.r[{id}]; return v || null; }})()"
    );
    let started = Instant::now();
    loop {
        tokio::time::sleep(Duration::from_millis(60)).await;
        let (tx, rx) = tokio::sync::oneshot::channel::<String>();
        let tx = std::sync::Mutex::new(Some(tx));
        w.eval_with_callback(take.clone(), move |r| {
            if let Some(tx) = tx.lock().ok().and_then(|mut t| t.take()) {
                let _ = tx.send(r);
            }
        })
        .map_err(|e| problem(e.to_string()))?;
        let raw = tokio::time::timeout(Duration::from_secs(5), rx).await.ok().and_then(|r| r.ok()).unwrap_or_default();
        // The page returns a JSON string, which arrives JSON-encoded once more.
        if let Some(inner) = serde_json::from_str::<Option<String>>(&raw).ok().flatten() {
            #[derive(Deserialize)]
            struct Reply {
                status: u16,
                body: String,
            }
            let reply: Reply = serde_json::from_str(&inner).map_err(|e| problem(e.to_string()))?;
            let json = serde_json::from_str(&reply.body).unwrap_or(Value::String(reply.body));
            return Ok((reply.status, json));
        }
        if started.elapsed() > REQUEST_TIMEOUT {
            return Err(problem("Roblox didn't answer in time."));
        }
    }
}

/// Roblox's error text, when it sent one.
fn roblox_error(body: &Value) -> Option<String> {
    body.get("errors")?
        .as_array()?
        .iter()
        .find_map(|e| e.get("message")?.as_str().map(str::to_owned))
        .or_else(|| body.get("message").and_then(|m| m.as_str()).map(str::to_owned))
}

/// A request to Roblox's avatar-related APIs through the signed-in page.
#[tauri::command]
pub async fn avatar_api(app: AppHandle, method: String, url: String, body: Option<Value>) -> Result<Value> {
    let parsed = url::Url::parse(&url).map_err(|_| problem("That isn't a valid address."))?;
    if !allowed_api(&parsed) {
        return Err(Problem::invalid(
            "Vesper's avatar tools only talk to Roblox's avatar services.",
            "This is a bug in Vesper.",
        ));
    }
    let method = method.to_ascii_uppercase();
    if !["GET", "POST", "PATCH", "DELETE"].contains(&method.as_str()) {
        return Err(Problem::invalid("Unsupported request.", "This is a bug in Vesper."));
    }
    if parsed.host_str() == Some("auth.roblox.com") && parsed.path() != "/v2/logout" {
        return Err(Problem::invalid("Vesper doesn't change sign-in settings.", "This is a bug in Vesper."));
    }
    let (status, json) = page_fetch(&app, &method, &parsed, body.as_ref()).await?;
    match status {
        200..=299 => Ok(json),
        401 => Err(Problem::new("avatar.signed_out", "You're not signed in to Roblox")
            .what("Roblox says this page isn't signed in.")
            .why("You haven't signed in here yet, or the sign-in expired.")
            .fix("Use “Sign in” on the Avatar page and sign in on Roblox's page.")),
        429 => Err(Problem::new("avatar.rate_limited", "Roblox asked to slow down")
            .what("Too many avatar changes in a short time.")
            .why("Roblox limits how often an avatar can be changed.")
            .fix("Wait a minute, then try again.")
            .action(ProblemAction::Retry)),
        0 => Err(problem(format!("The request didn't reach Roblox ({}).", json.as_str().unwrap_or("network error")))),
        s => Err(Problem::new("avatar.rejected", "Roblox didn't accept that")
            .what(roblox_error(&json).unwrap_or_else(|| format!("Roblox answered {s}.")))
            .why("Roblox checks every avatar change against its own rules (what you own, how many of each item can be worn).")
            .fix("Adjust the change and try again.")),
    }
}

/// Signs out on Roblox's side and removes the page's browsing data.
#[tauri::command]
pub async fn avatar_sign_out(app: AppHandle) -> Result<()> {
    if window(&app).is_some() {
        let logout = url::Url::parse("https://auth.roblox.com/v2/logout").expect("valid url");
        // Ends the session on Roblox's servers, not just here.
        let _ = page_fetch(&app, "POST", &logout, None).await;
        if let Some(w) = window(&app) {
            let _ = w.clear_all_browsing_data();
            let _ = w.close();
        }
    }
    log::info!("Signed out of Roblox in the avatar editor");
    Ok(())
}

#[cfg(windows)]
mod imp {
    use super::Bounds;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::*;

    /// Docks and shows `child`; true if it was hidden before.
    pub fn dock(parent: isize, child: isize, b: Bounds) -> bool {
        let (parent, child) = (HWND(parent as *mut _), HWND(child as *mut _));
        unsafe {
            let was_hidden = !IsWindowVisible(child).as_bool();
            if GetParent(child).ok() != Some(parent) {
                let style = GetWindowLongPtrW(child, GWL_STYLE) as u32;
                let docked = (style & !(WS_POPUP.0 | WS_CAPTION.0 | WS_THICKFRAME.0 | WS_SYSMENU.0))
                    | WS_CHILD.0
                    | WS_CLIPSIBLINGS.0;
                SetWindowLongPtrW(child, GWL_STYLE, docked as isize);
                let ex = GetWindowLongPtrW(child, GWL_EXSTYLE) as u32;
                SetWindowLongPtrW(child, GWL_EXSTYLE, (ex & !WS_EX_APPWINDOW.0) as isize);
                let _ = SetParent(child, Some(parent));
            }
            let _ = SetWindowPos(
                child,
                Some(HWND_TOP),
                b.x,
                b.y,
                b.width.max(1),
                b.height.max(1),
                SWP_FRAMECHANGED | SWP_SHOWWINDOW | SWP_NOACTIVATE,
            );
            was_hidden
        }
    }

    pub fn hide(child: isize) {
        unsafe {
            let _ = ShowWindow(HWND(child as *mut _), SW_HIDE);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_robloxs_avatar_services_can_be_called() {
        let ok = |u: &str| allowed_api(&url::Url::parse(u).unwrap());
        assert!(ok("https://avatar.roblox.com/v1/avatar"));
        assert!(ok("https://thumbnails.roblox.com/v1/users/avatar?userIds=1"));
        assert!(!ok("http://avatar.roblox.com/v1/avatar"));
        assert!(!ok("https://avatar.roblox.com.evil.example/v1/avatar"));
        assert!(!ok("https://economy.roblox.com.evil.example/"));
        assert!(!ok("https://apis.roblox.com/anything"));
        assert!(!ok("https://user:pass@avatar.roblox.com/"));
    }

    #[test]
    fn the_docked_page_stays_on_roblox() {
        let ok = |u: &str| allowed_page(&url::Url::parse(u).unwrap());
        assert!(ok("https://www.roblox.com/login"));
        assert!(ok("https://roblox.com/my/avatar"));
        assert!(ok("https://roblox-api.arkoselabs.com/fc/gt2/"));
        assert!(!ok("https://evil.example/roblox.com"));
        assert!(!ok("http://www.roblox.com/login"));
        assert!(!ok("https://www.roblox.com.evil.example/"));
    }

    #[test]
    fn roblox_error_messages_are_shown() {
        let body = serde_json::json!({ "errors": [{ "code": 0, "message": "Asset is not wearable" }] });
        assert_eq!(roblox_error(&body).as_deref(), Some("Asset is not wearable"));
        assert_eq!(roblox_error(&serde_json::json!({})), None);
    }
}
