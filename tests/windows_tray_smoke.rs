#![cfg(windows)]

use brewake::{
    config::RuntimeStatus,
    protocol::{CONTROL_ACTIVE_FOR, CONTROL_EXIT, CONTROL_OFF, CONTROL_ON, WM_CONTROL, WM_TRAY},
};
use std::{
    fs,
    mem::size_of,
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, WPARAM},
        UI::WindowsAndMessaging::{
            FindWindowW, GUI_POPUPMENUMODE, GUITHREADINFO, GetGUIThreadInfo,
            GetWindowThreadProcessId, PostMessageW, SMTO_ABORTIFHUNG, SendMessageTimeoutW,
            WM_CANCELMODE, WM_RBUTTONUP,
        },
    },
    core::{PCWSTR, w},
};

const TRAY_ID: usize = 1;
const TIMEOUT: Duration = Duration::from_secs(6);

/// Exercises the exact notification-area callback without relying on taskbar coordinates.
///
/// Ignored by default because it needs an interactive Windows desktop. Run explicitly with:
/// `cargo test --test windows_tray_smoke -- --ignored --nocapture`
#[test]
#[ignore = "requires an interactive Windows desktop"]
fn context_menu_survives_timer_reentry() {
    assert!(
        find_brewake_window().is_none(),
        "Close the running Brewake instance before this smoke test"
    );

    let local_app_data = unique_temp_dir();
    fs::create_dir(&local_app_data).expect("create isolated LOCALAPPDATA");
    let executable = std::env::var_os("BREWAKE_SMOKE_EXE")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_brewake").into());
    let child = Command::new(executable)
        .env("LOCALAPPDATA", &local_app_data)
        .spawn()
        .expect("launch Brewake");
    let mut app = TestApp::new(child, local_app_data);

    let hwnd =
        wait_for(TIMEOUT, find_brewake_window).expect("Brewake hidden window did not appear");
    let mut window_pid = 0;
    let thread_id = unsafe { GetWindowThreadProcessId(hwnd, Some(&mut window_pid)) };
    assert_ne!(thread_id, 0, "hidden window has no owning UI thread");
    assert_eq!(
        window_pid,
        app.id(),
        "found window belongs to another process"
    );

    post_control(hwnd, CONTROL_OFF, 0);
    post_control(hwnd, CONTROL_ACTIVE_FOR, 2);

    let runtime_path = app.local_app_data.join("Brewake").join("runtime.json");
    wait_for(TIMEOUT, || {
        read_status(&runtime_path)
            .filter(|status| status.mode == "active_for" && status.power_request_succeeded)
    })
    .expect("timed active state was not applied");

    unsafe {
        PostMessageW(
            Some(hwnd),
            WM_TRAY,
            WPARAM(TRAY_ID),
            LPARAM(WM_RBUTTONUP as isize),
        )
        .expect("post tray right-click callback");
    }
    wait_for(TIMEOUT, || menu_is_open(thread_id).then_some(()))
        .expect("native tray context menu did not enter popup-menu mode");

    // The state can only reach inactive here if WM_TIMER is dispatched successfully by
    // TrackPopupMenuEx's nested message loop. The old implementation aborted on this tick.
    let expired = wait_for(TIMEOUT, || {
        if !menu_is_open(thread_id) {
            return None;
        }
        read_status(&runtime_path).filter(|status| {
            !status.enabled && status.mode == "inactive" && status.deadline_unix_seconds.is_none()
        })
    })
    .expect("timer did not expire while the context menu remained open");
    assert_eq!(expired.mode, "inactive");
    assert!(app.is_running(), "Brewake aborted while its menu was open");

    // A second reentrant path: control messages must still be processed while the popup owns the
    // thread's nested message loop.
    post_control(hwnd, CONTROL_ON, 0);
    wait_for(TIMEOUT, || {
        if !menu_is_open(thread_id) {
            return None;
        }
        read_status(&runtime_path).filter(|status| {
            status.enabled && status.mode == "active" && status.power_request_succeeded
        })
    })
    .expect("control message was not processed while the context menu remained open");

    unsafe {
        let _ = SendMessageTimeoutW(
            hwnd,
            WM_CANCELMODE,
            WPARAM(0),
            LPARAM(0),
            SMTO_ABORTIFHUNG,
            2_000,
            None,
        );
    }
    wait_for(TIMEOUT, || (!menu_is_open(thread_id)).then_some(()))
        .expect("context menu did not close after WM_CANCELMODE");
    assert!(
        app.is_running(),
        "Brewake exited when the menu was dismissed"
    );

    post_control(hwnd, CONTROL_EXIT, 0);
    let status = app.wait_for_exit(TIMEOUT).expect("Brewake did not exit");
    assert!(status.success(), "Brewake exited with {status}");
    assert!(
        !runtime_path.exists(),
        "clean exit should remove the runtime snapshot"
    );
}

fn post_control(hwnd: HWND, command: usize, seconds: isize) {
    unsafe {
        PostMessageW(Some(hwnd), WM_CONTROL, WPARAM(command), LPARAM(seconds))
            .expect("post Brewake control message");
    }
}

fn find_brewake_window() -> Option<HWND> {
    unsafe { FindWindowW(w!("Brewake.TrayWindow.v1"), PCWSTR::null()).ok() }
}

fn menu_is_open(thread_id: u32) -> bool {
    let mut info = GUITHREADINFO {
        cbSize: size_of::<GUITHREADINFO>() as u32,
        ..Default::default()
    };
    unsafe { GetGUIThreadInfo(thread_id, &mut info) }.is_ok()
        && info.flags.contains(GUI_POPUPMENUMODE)
}

fn read_status(path: &Path) -> Option<RuntimeStatus> {
    serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
}

fn wait_for<T>(timeout: Duration, mut check: impl FnMut() -> Option<T>) -> Option<T> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(value) = check() {
            return Some(value);
        }
        if Instant::now() >= deadline {
            return None;
        }
        thread::sleep(Duration::from_millis(25));
    }
}

fn unique_temp_dir() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!("brewake-tray-test-{}-{nonce}", std::process::id()))
}

struct TestApp {
    child: Child,
    local_app_data: PathBuf,
}

impl TestApp {
    fn new(child: Child, local_app_data: PathBuf) -> Self {
        Self {
            child,
            local_app_data,
        }
    }

    fn id(&self) -> u32 {
        self.child.id()
    }

    fn is_running(&mut self) -> bool {
        self.child
            .try_wait()
            .expect("query child process")
            .is_none()
    }

    fn wait_for_exit(&mut self, timeout: Duration) -> Option<ExitStatus> {
        wait_for(timeout, || {
            self.child.try_wait().expect("query child process")
        })
    }
}

impl Drop for TestApp {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }

        let app_dir = self.local_app_data.join("Brewake");
        let _ = fs::remove_file(app_dir.join("runtime.json"));
        let _ = fs::remove_file(app_dir.join("config.json"));
        let _ = fs::remove_dir(&app_dir);
        let _ = fs::remove_dir(&self.local_app_data);
    }
}
