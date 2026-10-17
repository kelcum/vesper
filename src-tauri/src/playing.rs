//! What the player is doing in Roblox right now, and what follows from it:
//! the "Now playing" card, Discord's game status and the Recent games list.

use crate::discord::presence::GamePresence;
use crate::history::Visit;
use crate::roblox::activity::{Activity, ActivityWatcher, Change, ServerKind, Tracker};
use crate::roblox::web::{self, GameInfo, UserInfo};
use crate::state::{events, lock, AppState};
use serde::Serialize;
use std::time::SystemTime;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NowPlaying {
    pub place_id: u64,
    pub universe_id: Option<u64>,
    pub job_id: String,
    pub kind: ServerKind,
    pub joined_at: chrono::DateTime<chrono::Utc>,
    pub game: Option<GameInfo>,
    /// "London, GB", when server lookups are on and the server was found.
    pub location: Option<String>,
    /// Opens Roblox in this server; only for public servers.
    pub join_url: Option<String>,
    pub page_url: String,
}

pub fn now_playing(state: &AppState) -> Option<NowPlaying> {
    let a = lock(&state.activity).clone()?;
    let game = a.universe_id.and_then(|u| lock(&state.games).get(&u).cloned());
    let location = a.server_ip.as_ref().and_then(|ip| lock(&state.locations).get(ip).map(|l| l.short()));
    Some(NowPlaying {
        join_url: a.join_url(),
        page_url: a.page_url(),
        place_id: a.place_id,
        universe_id: a.universe_id,
        job_id: a.job_id,
        kind: a.kind,
        joined_at: a.joined_at,
        game,
        location,
    })
}

/// The signed-in player, from Roblox's log, for the profile link.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Player {
    pub id: u64,
    pub info: Option<UserInfo>,
    pub profile_url: String,
}

pub fn player(state: &AppState) -> Option<Player> {
    let id = (*lock(&state.roblox_user))?;
    Some(Player {
        id,
        info: lock(&state.user_info).clone().filter(|u| u.id == id),
        profile_url: UserInfo::profile_url(id),
    })
}

/// The game part of the Discord status, following the player's choices.
pub fn game_presence(state: &AppState) -> Option<GamePresence> {
    let s = lock(&state.settings).discord.clone();
    if !s.show_game {
        return None;
    }
    let a = lock(&state.activity).clone()?;
    let info = a.universe_id.and_then(|u| lock(&state.games).get(&u).cloned())?;
    Some(GamePresence {
        name: info.name,
        creator: info.creator,
        icon_url: info.icon_url,
        joined_unix: s.show_play_time.then(|| a.joined_at.timestamp()),
    })
}

fn emit(app: &AppHandle) {
    let state = app.state::<AppState>();
    let _ = app.emit(events::ACTIVITY, now_playing(&state));
}

fn emit_history(app: &AppHandle) {
    let state = app.state::<AppState>();
    let _ = app.emit(events::HISTORY, &*lock(&state.history));
}

fn save_history(state: &AppState) {
    if let Err(e) = lock(&state.history).save(&state.paths.history_file()) {
        log::warn!("Couldn't save the recent games list: {e}");
    }
}

fn history_on(state: &AppState) -> bool {
    lock(&state.settings).games.history
}

/// Copies what is known about the current server into its history entry.
fn fill_history(app: &AppHandle, a: &Activity) {
    let state = app.state::<AppState>();
    if !history_on(&state) {
        return;
    }
    let info = a.universe_id.and_then(|u| lock(&state.games).get(&u).cloned());
    let location = a.server_ip.as_ref().and_then(|ip| lock(&state.locations).get(ip).map(|l| l.short()));
    let changed = lock(&state.history).update(&a.job_id, |v: &mut Visit| {
        v.universe_id = v.universe_id.or(a.universe_id);
        if let Some(i) = &info {
            v.name = Some(i.name.clone());
            v.creator = Some(i.creator.clone());
            v.icon_url = i.icon_url.clone();
        }
        if location.is_some() {
            v.location = location.clone();
        }
    });
    if changed {
        save_history(&state);
        emit_history(app);
    }
}

fn on_change(app: &AppHandle, change: Change, tracker: &Tracker) {
    let state = app.state::<AppState>();
    let previous = lock(&state.activity).clone();
    *lock(&state.activity) = tracker.current.clone();
    if tracker.user_id.is_some() && *lock(&state.roblox_user) != tracker.user_id {
        *lock(&state.roblox_user) = tracker.user_id;
        fetch_user(app.clone());
    }
    match change {
        Change::Joined => {
            if let Some(p) = &previous {
                lock(&state.history).left(&p.job_id, chrono::Utc::now());
            }
            if let Some(a) = &tracker.current {
                log::info!("Joined a {:?} server of place {}", a.kind, a.place_id);
                if history_on(&state) {
                    lock(&state.history).joined(a);
                    save_history(&state);
                    emit_history(app);
                }
            }
        }
        Change::Left => {
            if let Some(p) = &previous {
                log::info!("Left place {}", p.place_id);
                lock(&state.history).left(&p.job_id, chrono::Utc::now());
                save_history(&state);
                emit_history(app);
            }
        }
        Change::Updated => {}
    }
    if let Some(a) = tracker.current.clone() {
        fill_history(app, &a);
        enrich(app.clone(), a);
    }
    emit(app);
    state.refresh_presence();
}

/// Looks up the game's name and icon, and the server's city, then updates everyone.
fn enrich(app: AppHandle, a: Activity) {
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let mut changed = false;
        if let Some(u) = a.universe_id {
            let known = lock(&state.games).contains_key(&u);
            if !known {
                match web::game_info(&state.http, u).await {
                    Ok(info) => {
                        lock(&state.games).insert(u, info);
                        changed = true;
                    }
                    Err(e) => log::info!("Couldn't look up the game: {e}"),
                }
            }
        }
        let locate = lock(&state.settings).games.locate_servers;
        if let (true, Some(ip)) = (locate, a.server_ip.clone()) {
            let known = lock(&state.locations).contains_key(&ip);
            if !known {
                match web::locate_server(&state.http, &ip).await {
                    Ok(loc) => {
                        log::info!("The server is in {}", loc.short());
                        lock(&state.locations).insert(ip, loc);
                        changed = true;
                    }
                    Err(e) => log::info!("Couldn't find where the server is: {e}"),
                }
            }
        }
        // Only act if the player is still in that server.
        let still = lock(&state.activity).as_ref().is_some_and(|c| c.job_id == a.job_id);
        if changed && still {
            let current = lock(&state.activity).clone();
            if let Some(c) = current {
                fill_history(&app, &c);
            }
            emit(&app);
            state.refresh_presence();
        }
    });
}

fn fetch_user(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let Some(id) = *lock(&state.roblox_user) else { return };
        if lock(&state.user_info).as_ref().is_some_and(|u| u.id == id) {
            return;
        }
        match web::user_info(&state.http, id).await {
            Ok(u) => {
                *lock(&state.user_info) = Some(u);
                let _ = app.emit(events::PLAYER, player(&state));
            }
            Err(e) => log::info!("Couldn't look up the Roblox account name: {e}"),
        }
    });
}

/// Starts following Roblox's log (Roblox just started, or was already running).
pub fn start(app: &AppHandle) {
    let state = app.state::<AppState>();
    let since = crate::roblox::process::snapshot()
        .ok()
        .and_then(|all| {
            crate::roblox::process::named(&all, crate::roblox::process::PLAYER_PROCESS)
                .filter_map(|p| crate::roblox::process::filetime_to_system(p.create_time))
                .min()
        })
        .unwrap_or_else(SystemTime::now);
    let handle = app.clone();
    match ActivityWatcher::start(state.env.roblox_logs(), since, move |c, t| on_change(&handle, c, t)) {
        Ok(w) => *lock(&state.activity_watch) = Some(w),
        Err(e) => log::warn!("Couldn't follow Roblox's log: {e}"),
    }
}

/// Roblox closed: stop following and close the open history entry.
pub fn stop(app: &AppHandle) {
    let state = app.state::<AppState>();
    lock(&state.activity_watch).take();
    let had = lock(&state.activity).take().is_some();
    lock(&state.history).close_all(chrono::Utc::now());
    if had {
        save_history(&state);
        emit_history(app);
    }
    emit(app);
}
