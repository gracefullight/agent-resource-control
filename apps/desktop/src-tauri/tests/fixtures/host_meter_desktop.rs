//! Opt-in noninteractive native visibility observation. Build with embedded
//! frontend assets: --features tauri/custom-protocol. Never run as a unit test.
use arc_desktop::{
    bridge::DesktopBridge,
    lifecycle::{self, DesktopState, LifecycleEvent, MAIN_WINDOW},
};
use serde_json::json;
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::Manager;

fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    assert_eq!(
        args.len(),
        2,
        "usage: host-meter-desktop TARGET_RECEIPT_JSONL DURATION_SECONDS"
    );
    let path = PathBuf::from(&args[0]);
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap()
        .canonicalize()
        .unwrap();
    let parent = path.parent().unwrap().canonicalize().unwrap();
    assert!(
        parent.starts_with(root.join("target/host-meter")),
        "receipt must be under target/host-meter"
    );
    let duration: u64 = args[1].to_str().unwrap().parse().unwrap();
    assert!((1..=900).contains(&duration));
    let file = Arc::new(Mutex::new(
        OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path)
            .unwrap(),
    ));
    let epoch = Instant::now();
    let app = arc_desktop::configure(tauri::Builder::default(), DesktopBridge::new())
        .build(tauri::generate_context!())
        .expect("native desktop prerequisite");
    app.run(move |handle, event| {
        let ready = matches!(event, tauri::RunEvent::Ready);
        lifecycle::on_run_event(handle, event);
        if ready {
            observe(handle, &file, epoch, "before_hide");
            handle
                .state::<DesktopState>()
                .dispatch(LifecycleEvent::CloseRequested)
                .expect("existing native hide lifecycle");
            observe(handle, &file, epoch, "after_hide");
            let app = handle.clone();
            let file = file.clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_secs(1));
                let app = app.clone();
                let file = file.clone();
                let for_main = app.clone();
                app.run_on_main_thread(move || {
                    observe(&for_main, &file, epoch, "hidden_interval");
                    if epoch.elapsed() >= Duration::from_secs(duration) {
                        for_main
                            .state::<DesktopState>()
                            .dispatch(LifecycleEvent::QuitDesktop)
                            .expect("desktop-only quit");
                    }
                })
                .expect("native event loop still alive");
                if epoch.elapsed() >= Duration::from_secs(duration) {
                    break;
                }
            });
        }
    });
}

fn observe(app: &tauri::AppHandle, file: &Mutex<File>, epoch: Instant, phase: &str) {
    let window = app
        .get_webview_window(MAIN_WINDOW)
        .expect("actual native main window");
    let record = json!({
        "pid":std::process::id(), "atMs":SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis(),
        "monotonicMs":epoch.elapsed().as_millis(), "phase":phase,"windowLabel":window.label(),
        "isVisible":window.is_visible().expect("native window visibility"),
        "isMinimized":window.is_minimized().expect("native minimized state"),
        "runtime":"production_wry", "lifecycle":"DesktopState::dispatch(CloseRequested) -> TauriShell::hide_main",
    });
    let mut file = file.lock().unwrap();
    writeln!(file, "{record}").unwrap();
    file.flush().unwrap();
}
