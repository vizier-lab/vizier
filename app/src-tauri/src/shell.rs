//! The Tauri glue: window, tray, plugins, commands and the `vizier-asset` scheme. All logic
//! lives in the modules next to it; this file only translates. It is compiled only with the
//! `shell` feature and was written against the Tauri 2 API without being built where there
//! are no webview libraries (see tasks.md T056 for the on-device check).

use std::{borrow::Cow, sync::Arc};

use serde_json::{Value, json};
use tauri::{
    AppHandle, Emitter, Manager, RunEvent, State, WebviewUrl, WebviewWindowBuilder, WindowEvent,
    ipc::{Channel, InvokeBody, Request},
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_deep_link::DeepLinkExt;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_updater::UpdaterExt;

use crate::{
    desktop::{self, Navigator},
    engine::{CommandError, Engine, SettingsPatch},
    ipc::{self, IpcError, Topics},
    settings::DesktopSettings,
};

struct Shell {
    engine: Engine,
    topics: Arc<Topics>,
    nav: Arc<Navigator>,
    quitting: std::sync::atomic::AtomicBool,
}

fn focus_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Shows the window and sends the WebUI to `href` (held until it is listening).
fn navigate(app: &AppHandle, href: String) {
    focus_main(app);
    let shell = app.state::<Shell>();
    if let Some(href) = shell.nav.request(href) {
        let _ = app.emit("desktop://navigate", json!({ "href": href }));
    }
}

fn handle_urls<'a>(app: &AppHandle, urls: impl IntoIterator<Item = &'a str>) {
    for url in urls {
        if let Some(href) = desktop::deep_link_href(url) {
            navigate(app, href);
        }
    }
}

async fn quit(app: AppHandle) {
    let shell = app.state::<Shell>();
    shell.quitting.store(true, std::sync::atomic::Ordering::SeqCst);
    shell.topics.close_all();
    shell.engine.shutdown().await;
    app.exit(0);
}

// ---------------------------------------------------------------- commands: api, assets, topics

#[tauri::command]
async fn api(request: Request<'_>, shell: State<'_, Shell>) -> Result<Value, IpcError> {
    let header = |name: &str| {
        request.headers().get(name).and_then(|v| v.to_str().ok()).map(str::to_string)
    };
    let method = header("x-method").unwrap_or_else(|| "GET".into());
    let path = header("x-path").unwrap_or_default();
    let body = match request.body() {
        InvokeBody::Raw(bytes) => bytes.clone(),
        InvokeBody::Json(value) if value.is_null() => vec![],
        InvokeBody::Json(value) => value.to_string().into_bytes(),
    };
    let reply = ipc::api(&shell.engine, &method, &path, header("content-type").as_deref(), body).await?;
    Ok(json!({ "status": reply.status, "body": reply.body }))
}

#[tauri::command]
async fn topic_open(
    agent_id: String,
    channel_id: String,
    topic_id: String,
    frames: Channel<Value>,
    shell: State<'_, Shell>,
) -> Result<Value, Value> {
    match shell
        .topics
        .open(&shell.engine, &agent_id, &channel_id, &topic_id, move |frame| {
            let _ = frames.send(frame);
        })
        .await
    {
        Ok(id) => Ok(json!({ "id": id })),
        Err(refused) => Err(json!({ "status": refused.status })),
    }
}

#[tauri::command]
async fn topic_send(id: u64, frame: Value, shell: State<'_, Shell>) -> Result<(), ()> {
    shell.topics.send(id, frame).await;
    Ok(())
}

#[tauri::command]
fn topic_close(id: u64, shell: State<'_, Shell>) {
    shell.topics.close(id);
}

// ---------------------------------------------------------------- commands: desktop shell

#[tauri::command]
fn desktop_state(shell: State<'_, Shell>) -> Value {
    shell.engine.state().to_json()
}

#[tauri::command]
async fn desktop_setup_owner(username: String, password: Option<String>, shell: State<'_, Shell>) -> Result<(), CommandError> {
    shell.engine.setup_owner(&username, password.as_deref()).await
}

#[tauri::command]
async fn desktop_choose_owner(user_id: String, shell: State<'_, Shell>) -> Result<(), CommandError> {
    shell.engine.choose_owner(&user_id).await
}

#[tauri::command]
async fn desktop_open_workspace(path: Option<String>, app: AppHandle, shell: State<'_, Shell>) -> Result<(), CommandError> {
    let chosen = match path {
        Some(p) => Some(std::path::PathBuf::from(p)),
        None => {
            let app = app.clone();
            tauri::async_runtime::spawn_blocking(move || {
                app.dialog().file().blocking_pick_folder().and_then(|p| p.into_path().ok())
            })
            .await
            .map_err(|e| CommandError::new("dialog", e.to_string()))?
        }
    };
    let Some(path) = chosen else { return Ok(()) };
    shell.topics.close_all();
    shell.engine.open_workspace(path).await;
    Ok(())
}

#[tauri::command]
async fn runtime_restart(shell: State<'_, Shell>) -> Result<(), ()> {
    shell.topics.close_all();
    shell.engine.restart().await;
    Ok(())
}

#[tauri::command]
fn desktop_settings_get(shell: State<'_, Shell>) -> Value {
    let s = shell.engine.settings();
    json!({ "workspace": s.workspace, "close_to_tray": s.close_to_tray, "web_access": s.web_access, "updates": s.updates })
}

#[tauri::command]
async fn desktop_settings_set(patch: SettingsPatch, shell: State<'_, Shell>) -> Result<Value, CommandError> {
    let s = shell.engine.settings_set(patch).await?;
    Ok(json!({ "workspace": s.workspace, "close_to_tray": s.close_to_tray, "web_access": s.web_access, "updates": s.updates }))
}

#[tauri::command]
fn desktop_ready(app: AppHandle, shell: State<'_, Shell>) {
    if let Some(href) = shell.nav.ready() {
        let _ = app.emit("desktop://navigate", json!({ "href": href }));
    }
}

#[cfg(debug_assertions)]
#[tauri::command]
async fn runtime_debug_stop_scheduler(shell: State<'_, Shell>) -> Result<bool, ()> {
    Ok(shell.engine.run_debug_stop("scheduler").await)
}

#[tauri::command]
async fn save_file(path: String, filename: String, app: AppHandle, shell: State<'_, Shell>) -> Result<(), String> {
    let picker = app.clone();
    let name = desktop::safe_filename(&filename);
    let dest = tauri::async_runtime::spawn_blocking(move || {
        picker.dialog().file().set_file_name(name).blocking_save_file().and_then(|p| p.into_path().ok())
    })
    .await
    .map_err(|e| e.to_string())?;
    match dest {
        Some(dest) => desktop::save_file(&shell.engine, &path, &dest).await,
        None => Ok(()),
    }
}

#[tauri::command]
async fn open_file(path: String, filename: String, app: AppHandle, shell: State<'_, Shell>) -> Result<(), String> {
    let file = desktop::open_file_path(&shell.engine, &path, &filename).await?;
    app.opener().open_path(file.to_string_lossy(), None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
fn notify(title: String, body: String, href: String, app: AppHandle, shell: State<'_, Shell>) -> Result<(), String> {
    // Desktop platforms do not report a click on a notification, so remember it and open the
    // notifications panel when the window next gains focus (research R11).
    let _ = href;
    shell.nav.note_notification();
    app.notification().builder().title(title).body(body).show().map_err(|e| e.to_string())
}

#[tauri::command]
async fn update_check(app: AppHandle) -> Result<Value, String> {
    let updater = app.updater().map_err(|e| e.to_string())?;
    match updater.check().await.map_err(|e| e.to_string())? {
        Some(update) => Ok(json!({ "available": true, "version": update.version, "notes": update.body })),
        None => Ok(json!({ "available": false })),
    }
}

#[tauri::command]
async fn update_install(app: AppHandle, shell: State<'_, Shell>) -> Result<(), String> {
    let updater = app.updater().map_err(|e| e.to_string())?;
    let Some(update) = updater.check().await.map_err(|e| e.to_string())? else { return Ok(()) };
    update.download_and_install(|_, _| {}, || {}).await.map_err(|e| e.to_string())?;
    shell.topics.close_all();
    shell.engine.shutdown().await;
    app.restart();
}

// ---------------------------------------------------------------- the vizier-asset:// scheme

fn asset_response(status: u16, headers: Vec<(String, String)>, body: Vec<u8>) -> tauri::http::Response<Cow<'static, [u8]>> {
    let mut builder = tauri::http::Response::builder()
        .status(status)
        .header("access-control-allow-origin", "*");
    for (name, value) in headers {
        builder = builder.header(name, value);
    }
    builder.body(Cow::Owned(body)).unwrap_or_else(|_| tauri::http::Response::new(Cow::Owned(vec![])))
}

// ---------------------------------------------------------------- entry point

pub fn run() {
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            focus_main(app);
            handle_urls(app, argv.iter().map(String::as_str));
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init());

    builder = builder.register_asynchronous_uri_scheme_protocol("vizier-asset", |ctx, request, responder| {
        let app = ctx.app_handle().clone();
        tauri::async_runtime::spawn(async move {
            let shell = app.state::<Shell>();
            let path = request
                .uri()
                .path_and_query()
                .map(|p| p.as_str().to_string())
                .unwrap_or_default();
            let range = request.headers().get("range").and_then(|v| v.to_str().ok()).map(str::to_string);
            let response = match ipc::asset(&shell.engine, request.method().as_str(), &path, range.as_deref()).await {
                Ok(reply) => asset_response(reply.status, reply.headers, reply.body),
                Err(_) => asset_response(503, vec![], b"engine_not_ready".to_vec()),
            };
            responder.respond(response);
        });
    });

    #[cfg(debug_assertions)]
    let handler: Box<dyn Fn(tauri::ipc::Invoke<tauri::Wry>) -> bool + Send + Sync> = Box::new(tauri::generate_handler![
        api, topic_open, topic_send, topic_close, desktop_state, desktop_setup_owner,
        desktop_choose_owner, desktop_open_workspace, runtime_restart, desktop_settings_get,
        desktop_settings_set, desktop_ready, save_file, open_file, notify, update_check,
        update_install, runtime_debug_stop_scheduler
    ]);
    #[cfg(not(debug_assertions))]
    let handler: Box<dyn Fn(tauri::ipc::Invoke<tauri::Wry>) -> bool + Send + Sync> = Box::new(tauri::generate_handler![
        api, topic_open, topic_send, topic_close, desktop_state, desktop_setup_owner,
        desktop_choose_owner, desktop_open_workspace, runtime_restart, desktop_settings_get,
        desktop_settings_set, desktop_ready, save_file, open_file, notify, update_check,
        update_install
    ]);

    let app = builder
        .invoke_handler(handler)
        .setup(|app| {
            let handle = app.handle().clone();
            let config_dir = app.path().app_config_dir()?;
            let data_dir = app.path().app_data_dir()?;
            let settings = DesktopSettings::load(&config_dir, &data_dir);
            let engine = Engine::new(config_dir, settings.clone());
            app.manage(Shell {
                engine: engine.clone(),
                topics: Arc::new(Topics::default()),
                nav: Arc::new(Navigator::default()),
                quitting: Default::default(),
            });

            // Engine state and web access changes go to the window as events.
            let (h, mut states) = (handle.clone(), engine.subscribe_state());
            tauri::async_runtime::spawn(async move {
                while let Ok(state) = states.recv().await {
                    let _ = h.emit("desktop://state", state.to_json());
                }
            });
            let (h, mut web) = (handle.clone(), engine.subscribe_web_access());
            tauri::async_runtime::spawn(async move {
                while let Ok(status) = web.recv().await {
                    let _ = h.emit("desktop://web-access", &status);
                }
            });
            tauri::async_runtime::spawn(async move { engine.start().await });

            // Deep links: `vizier://…` focuses the window and navigates.
            #[cfg(any(target_os = "linux", all(debug_assertions, windows)))]
            let _ = app.deep_link().register_all();
            let h = handle.clone();
            app.deep_link().on_open_url(move |event| {
                let urls: Vec<String> = event.urls().iter().map(|u| u.to_string()).collect();
                handle_urls(&h, urls.iter().map(String::as_str));
            });

            // The main window is built here, not in tauri.conf.json, to guard navigation (FR-013).
            let guard = handle.clone();
            let dev_url = app.config().build.dev_url.as_ref().map(|u| u.to_string());
            WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
                .title("Vizier")
                .inner_size(1280.0, 800.0)
                .min_inner_size(900.0, 600.0)
                .disable_drag_drop_handler()
                .on_navigation(move |url| {
                    if desktop::navigation_allowed(url.as_str(), dev_url.as_deref()) {
                        return true;
                    }
                    if matches!(url.scheme(), "http" | "https" | "mailto") {
                        let _ = guard.opener().open_url(url.as_str(), None::<&str>);
                    }
                    false
                })
                .build()?;

            // Tray: Open / Quit; a left click shows the window.
            let open = MenuItem::with_id(app, "open", "Open Vizier", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &quit_item])?;
            let mut tray = TrayIconBuilder::new()
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "open" => focus_main(app),
                    "quit" => {
                        let app = app.clone();
                        tauri::async_runtime::spawn(quit(app));
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                        focus_main(tray.app_handle());
                    }
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            let app = window.app_handle();
            let shell = app.state::<Shell>();
            match event {
                WindowEvent::CloseRequested { api, .. } => {
                    if shell.engine.settings().close_to_tray {
                        api.prevent_close();
                        let _ = window.hide();
                    } else {
                        api.prevent_close();
                        let app = app.clone();
                        tauri::async_runtime::spawn(quit(app));
                    }
                }
                WindowEvent::Focused(true) => {
                    if let Some(href) = shell.nav.on_focus() {
                        let _ = app.emit("desktop://navigate", json!({ "href": href }));
                    }
                }
                WindowEvent::Destroyed => shell.topics.close_all(),
                _ => {}
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building the vizier app");

    app.run(|app, event| {
        if let RunEvent::ExitRequested { api, .. } = event {
            let shell = app.state::<Shell>();
            // Closing the last window must not quit while close-to-tray keeps the engine up.
            if !shell.quitting.load(std::sync::atomic::Ordering::SeqCst) {
                api.prevent_exit();
            }
        }
    });
}
