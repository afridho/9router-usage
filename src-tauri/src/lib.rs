mod router;
pub mod settings;

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};
use tauri_plugin_notification::NotificationExt;

pub struct RouterState {
    client: Arc<tokio::sync::Mutex<Option<router::RouterClient>>>,
    data: Arc<Mutex<router::RouterData>>,
}
pub struct AlertedSet(pub Arc<Mutex<HashSet<String>>>);
static IS_PINNED: AtomicBool = AtomicBool::new(false);

#[tauri::command]
async fn open_url(url: String) {
    let _ = open::that(url);
}
#[tauri::command]
fn toggle_pin(window: tauri::WebviewWindow) -> bool {
    let v = !IS_PINNED.load(Ordering::Relaxed);
    IS_PINNED.store(v, Ordering::Relaxed);
    let _ = window.set_always_on_top(v);
    v
}
#[tauri::command]
fn hide_window(window: tauri::WebviewWindow) {
    let _ = window.hide();
}
#[tauri::command]
fn set_window_height(window: tauri::WebviewWindow, height: f64) {
    let max_height = window
        .current_monitor()
        .ok()
        .flatten()
        .map(|monitor| monitor.work_area().size.height as f64 / monitor.scale_factor() - 16.0)
        .unwrap_or(720.0);
    let height = height.clamp(260.0, max_height);
    let _ = window.set_size(tauri::LogicalSize::new(380.0, height));
    position_window(&window);
}
#[tauri::command]
fn get_router_data(state: tauri::State<'_, RouterState>) -> router::RouterData {
    state.data.lock().unwrap().clone()
}

async fn ensure_client(state: &RouterState) -> Result<(), String> {
    let url = settings::read_settings().server_url;
    let mut guard = state.client.lock().await;
    if guard.is_none() {
        *guard = Some(router::RouterClient::new(&url)?);
    }
    Ok(())
}
#[tauri::command]
async fn router_login(
    app: tauri::AppHandle,
    state: tauri::State<'_, RouterState>,
    server_url: String,
    password: String,
) -> Result<router::RouterData, String> {
    let mut s = settings::read_settings();
    let previous_server_url = s.server_url.clone();
    s.server_url = server_url.trim().trim_end_matches('/').into();
    let client = router::RouterClient::new(&s.server_url)?;
    client.login(&password).await?;
    settings::write_settings(&s)?;
    if previous_server_url != s.server_url {
        settings::delete_password(&previous_server_url);
    }
    if s.remember_password {
        settings::save_password(&s.server_url, &password)?;
    } else {
        settings::delete_password(&s.server_url);
    }
    let data = client.fetch(true).await;
    *state.client.lock().await = Some(client);
    *state.data.lock().unwrap() = data.clone();
    let _ = app.emit("router-updated", data.clone());
    Ok(data)
}
#[tauri::command]
async fn router_logout(
    app: tauri::AppHandle,
    state: tauri::State<'_, RouterState>,
) -> Result<(), String> {
    let mut g = state.client.lock().await;
    let credential_url = g
        .as_ref()
        .map(|client| client.base_url().to_string())
        .unwrap_or_else(|| settings::read_settings().server_url);
    if let Some(c) = g.as_ref() {
        c.logout().await;
    }
    *g = None;
    settings::delete_password(&credential_url);
    let d = router::RouterData::default();
    *state.data.lock().unwrap() = d.clone();
    let _ = app.emit("router-updated", d);
    Ok(())
}
#[tauri::command]
async fn refresh_router(
    app: tauri::AppHandle,
    state: tauri::State<'_, RouterState>,
    force: bool,
) -> Result<router::RouterData, String> {
    ensure_client(&state).await?;
    let g = state.client.lock().await;
    let d = g.as_ref().unwrap().fetch(force).await;
    drop(g);
    *state.data.lock().unwrap() = d.clone();
    check_notifications(&app, &d);
    let _ = app.emit("router-updated", d.clone());
    Ok(d)
}

fn check_notifications(app: &tauri::AppHandle, data: &router::RouterData) {
    let settings = settings::read_settings();
    if !settings.enable_notifications {
        return;
    }
    let alerted = app.state::<AlertedSet>().0.clone();
    let mut set = alerted.lock().unwrap();
    for a in &data.accounts {
        if !a.active {
            continue;
        }
        for q in &a.quotas {
            if q.is_credit_balance {
                if let Some(balance) = q.total {
                    let key = format!("{}:{}:credit", a.connection_id, q.id);
                    if balance <= settings.credit_threshold && !set.contains(&key) {
                        let currency = q.currency.as_deref().unwrap_or("USD");
                        let _ = app
                            .notification()
                            .builder()
                            .title(format!("{} · {}", a.provider_name, q.label))
                            .body(format!("Credit remaining: {balance:.2} {currency}"))
                            .show();
                        set.insert(key);
                    } else if balance > settings.credit_threshold {
                        set.remove(&key);
                    }
                }
                continue;
            }
            let remaining = q.remaining_percent.or_else(|| match (q.used, q.total) {
                (Some(u), Some(t)) if t > 0.0 => Some((t - u) / t * 100.0),
                _ => None,
            });
            if let Some(p) = remaining {
                for threshold in &settings.quota_thresholds {
                    let key = format!("{}:{}:{}", a.connection_id, q.id, threshold);
                    if p <= *threshold && !set.contains(&key) {
                        let _ = app
                            .notification()
                            .builder()
                            .title(format!("{} · {}", a.provider_name, q.label))
                            .body(format!("{p:.0}% quota remaining"))
                            .show();
                        set.insert(key);
                    } else if p > *threshold {
                        set.remove(&key);
                    }
                }
            }
        }
    }
}
fn position_window(window: &tauri::WebviewWindow) {
    if let Some(m) = window.primary_monitor().ok().flatten() {
        let wa = m.work_area();
        let sz = window
            .outer_size()
            .unwrap_or(tauri::PhysicalSize::new(380, 650));
        let _ = window.set_position(tauri::PhysicalPosition::new(
            wa.position.x + wa.size.width as i32 - sz.width as i32 - 12,
            wa.position.y + wa.size.height as i32 - sz.height as i32 - 8,
        ));
    }
}
fn toggle_window(window: &tauri::WebviewWindow) {
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
    } else {
        position_window(window);
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                position_window(&window);
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .plugin(tauri_plugin_opener::init())
        .manage(RouterState {
            client: Arc::new(tokio::sync::Mutex::new(None)),
            data: Arc::new(Mutex::new(router::RouterData::default())),
        })
        .manage(AlertedSet(Arc::new(Mutex::new(HashSet::new()))))
        .invoke_handler(tauri::generate_handler![
            hide_window,
            set_window_height,
            toggle_pin,
            open_url,
            get_router_data,
            router_login,
            router_logout,
            refresh_router,
            settings::get_settings,
            settings::save_settings
        ])
        .setup(|app| {
            let poll_app = app.handle().clone();
            let client = app.state::<RouterState>().client.clone();
            let data = app.state::<RouterState>().data.clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    let s = settings::read_settings();
                    {
                        let mut g = client.lock().await;
                        if g.is_none() {
                            *g = router::RouterClient::new(&s.server_url).ok();
                            if let (Some(c), Some(password)) = (
                                g.as_ref(),
                                if s.remember_password {
                                    settings::read_password(&s.server_url)
                                } else {
                                    None
                                },
                            ) {
                                let _ = c.login(&password).await;
                            }
                        }
                        if let Some(c) = g.as_ref() {
                            let mut d = c.fetch(false).await;
                            if !d.authenticated {
                                if let Some(password) = if s.remember_password {
                                    settings::read_password(&s.server_url)
                                } else {
                                    None
                                } {
                                    if c.login(&password).await.is_ok() {
                                        d = c.fetch(false).await;
                                    }
                                }
                            }
                            *data.lock().unwrap() = d.clone();
                            check_notifications(&poll_app, &d);
                            let _ = poll_app.emit("router-updated", d);
                        }
                    }
                    tokio::time::sleep(Duration::from_secs(s.refresh_interval_seconds.max(60)))
                        .await;
                }
            });
            let lost: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));
            let lost_tray = lost.clone();
            let show = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            TrayIconBuilder::new()
                .icon(Image::from_bytes(include_bytes!("../icons/32x32.png"))?)
                .menu(&menu)
                .show_menu_on_left_click(false)
                .tooltip("9Router Usage")
                .on_menu_event(|app, e| match e.id.as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            toggle_window(&w)
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(move |tray, e| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = e
                    {
                        let recent = lost_tray
                            .lock()
                            .ok()
                            .and_then(|v| *v)
                            .map(|t| t.elapsed().as_millis() < 300)
                            .unwrap_or(false);
                        if !recent {
                            if let Some(w) = tray.app_handle().get_webview_window("main") {
                                toggle_window(&w)
                            }
                        }
                    }
                })
                .build(app)?;
            let w = app.get_webview_window("main").unwrap();
            if !std::env::args().any(|arg| arg == "--autostart") {
                position_window(&w);
                let _ = w.show();
                let _ = w.set_focus();
            }
            let wc = w.clone();
            w.on_window_event(move |e| {
                if let tauri::WindowEvent::Focused(false) = e {
                    *lost.lock().unwrap() = Some(Instant::now());
                    if !IS_PINNED.load(Ordering::Relaxed) {
                        let _ = wc.hide();
                    }
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running 9Router Usage");
}
