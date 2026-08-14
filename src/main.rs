#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(not(windows))]
fn main() {
    eprintln!("Brewake currently supports Windows only.");
}

#[cfg(windows)]
fn main() {
    if let Err(error) = windows_app::run() {
        windows_app::show_fatal_error(&error.to_string());
    }
}

#[cfg(windows)]
mod windows_app {
    use brewake::{
        config::{Config, RuntimeStatus, clear_runtime, load_config, save_config, save_runtime},
        protocol::{
            CONTROL_ACTIVE_FOR, CONTROL_EXIT, CONTROL_OFF, CONTROL_ON, CONTROL_PAUSE_FOR,
            CONTROL_TOGGLE, MAX_TIMER_SECONDS, WM_CONTROL, WM_TRAY,
        },
        state::AwakeState,
    };
    use std::{
        cell::RefCell,
        mem::size_of,
        os::windows::ffi::OsStrExt,
        path::Path,
        slice,
        sync::atomic::{AtomicU32, Ordering},
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };
    use windows::{
        Win32::{
            Foundation::{
                CloseHandle, ERROR_ALREADY_EXISTS, ERROR_SUCCESS, GetLastError, HINSTANCE, HWND,
                LPARAM, LRESULT, POINT, WPARAM,
            },
            System::{
                LibraryLoader::GetModuleHandleW,
                Power::{
                    ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED, SetThreadExecutionState,
                },
                Registry::{
                    HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SZ, RegCloseKey,
                    RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
                },
                Threading::CreateMutexW,
            },
            UI::{
                Shell::{
                    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
                    NOTIFYICONDATAW, Shell_NotifyIconW,
                },
                WindowsAndMessaging::{
                    AppendMenuW, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, CreateIconFromResourceEx,
                    CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon, DestroyMenu,
                    DestroyWindow, DispatchMessageW, FindWindowW, GetCursorPos, GetMessageW, HICON,
                    HMENU, IDC_ARROW, LR_DEFAULTCOLOR, LoadCursorW, MB_ICONERROR,
                    MB_ICONINFORMATION, MB_OK, MENU_ITEM_FLAGS, MF_CHECKED, MF_DISABLED, MF_GRAYED,
                    MF_POPUP, MF_SEPARATOR, MF_STRING, MSG, MessageBoxW, PBT_APMPOWERSTATUSCHANGE,
                    PBT_APMRESUMEAUTOMATIC, PostMessageW, PostQuitMessage, RegisterClassW,
                    RegisterWindowMessageW, SetForegroundWindow, SetTimer, TPM_BOTTOMALIGN,
                    TPM_LEFTALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenuEx,
                    TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE, WM_DESTROY,
                    WM_LBUTTONUP, WM_NULL, WM_POWERBROADCAST, WM_RBUTTONUP, WM_TIMER, WNDCLASSW,
                },
            },
        },
        core::{PCWSTR, Result as WinResult, w},
    };

    const TRAY_ID: u32 = 1;
    const TIMER_ID: usize = 1;

    const MENU_TOGGLE: usize = 100;
    const MENU_ACTIVE_15M: usize = 111;
    const MENU_ACTIVE_30M: usize = 112;
    const MENU_ACTIVE_1H: usize = 113;
    const MENU_ACTIVE_2H: usize = 114;
    const MENU_ACTIVE_4H: usize = 115;
    const MENU_ACTIVE_8H: usize = 116;
    const MENU_PAUSE_15M: usize = 211;
    const MENU_PAUSE_30M: usize = 212;
    const MENU_PAUSE_1H: usize = 213;
    const MENU_PAUSE_2H: usize = 214;
    const MENU_PAUSE_4H: usize = 215;
    const MENU_DISPLAY: usize = 300;
    const MENU_STARTUP: usize = 400;
    const MENU_STATUS: usize = 500;
    const MENU_ABOUT: usize = 501;
    const MENU_EXIT: usize = 900;

    const RUN_KEY: windows::core::PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
    static TASKBAR_CREATED_MESSAGE: AtomicU32 = AtomicU32::new(0);

    thread_local! {
        static APP: RefCell<Option<App>> = const { RefCell::new(None) };
    }

    pub fn run() -> WinResult<()> {
        let mutex = unsafe { CreateMutexW(None, false, w!("Local\\Brewake.SingleInstance.v1"))? };
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            // A normal second launch simply toggles the existing instance, matching common tray
            // utility behavior. Explicit commands are handled by `brewakectl`.
            if let Ok(hwnd) = unsafe { FindWindowW(w!("Brewake.TrayWindow.v1"), PCWSTR::null()) } {
                let _ = unsafe {
                    PostMessageW(Some(hwnd), WM_CONTROL, WPARAM(CONTROL_TOGGLE), LPARAM(0))
                };
            }
            unsafe { CloseHandle(mutex)? };
            return Ok(());
        }

        let instance = HINSTANCE(unsafe { GetModuleHandleW(None)? }.0);
        let class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            hCursor: unsafe { LoadCursorW(None, IDC_ARROW)? },
            lpszClassName: w!("Brewake.TrayWindow.v1"),
            ..Default::default()
        };
        if unsafe { RegisterClassW(&class) } == 0 {
            unsafe { CloseHandle(mutex)? };
            return Err(windows::core::Error::from_thread());
        }

        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("Brewake.TrayWindow.v1"),
                w!("Brewake"),
                WINDOW_STYLE::default(),
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                0,
                0,
                None,
                None,
                Some(instance),
                None,
            )?
        };

        let config = load_config();
        TASKBAR_CREATED_MESSAGE.store(
            unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) },
            Ordering::Relaxed,
        );
        let mut app = App::new(hwnd, config);
        app.add_tray_icon()?;
        app.apply_and_refresh(true);
        APP.with(|slot| *slot.borrow_mut() = Some(app));

        unsafe { SetTimer(Some(hwnd), TIMER_ID, 1_000, None) };

        let mut message = MSG::default();
        while unsafe { GetMessageW(&mut message, None, 0, 0) }.as_bool() {
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }

        APP.with(|slot| {
            if let Some(mut app) = slot.borrow_mut().take() {
                app.shutdown();
            }
        });
        unsafe { CloseHandle(mutex)? };
        Ok(())
    }

    pub fn show_fatal_error(message: &str) {
        let text = to_wide(&format!("Brewake could not start.\n\n{message}"));
        unsafe {
            MessageBoxW(
                None,
                PCWSTR(text.as_ptr()),
                w!("Brewake"),
                MB_OK | MB_ICONERROR,
            );
        }
    }

    struct App {
        hwnd: HWND,
        state: AwakeState,
        config: Config,
        icon: Option<HICON>,
        tray_added: bool,
        last_reapply: Instant,
        deadline_unix_seconds: Option<u64>,
        power_request_succeeded: bool,
    }

    struct MenuSnapshot {
        hwnd: HWND,
        status: String,
        enabled: bool,
        keep_display_on: bool,
        startup_enabled: bool,
    }

    enum UiAction {
        None,
        Message {
            hwnd: HWND,
            body: String,
            title: &'static str,
        },
        Exit(HWND),
    }

    impl App {
        fn new(hwnd: HWND, config: Config) -> Self {
            Self {
                hwnd,
                state: AwakeState::new(config.enabled, config.keep_display_on),
                config,
                icon: None,
                tray_added: false,
                last_reapply: Instant::now(),
                deadline_unix_seconds: None,
                power_request_succeeded: false,
            }
        }

        fn add_tray_icon(&mut self) -> WinResult<()> {
            let icon = create_mug_icon(self.state.enabled(), self.state.is_timed())?;
            let data = self.notify_data(icon, self.tooltip());
            if !unsafe { Shell_NotifyIconW(NIM_ADD, &data) }.as_bool() {
                unsafe { DestroyIcon(icon)? };
                return Err(windows::core::Error::from_thread());
            }
            if let Some(old) = self.icon.replace(icon) {
                let _ = unsafe { DestroyIcon(old) };
            }
            self.tray_added = true;
            Ok(())
        }

        fn apply_and_refresh(&mut self, replace_icon: bool) {
            self.apply_power_request();
            self.refresh_tray(replace_icon);
            self.save_runtime_status();
        }

        fn apply_power_request(&mut self) {
            let mut flags = ES_CONTINUOUS;
            if self.state.enabled() {
                flags |= ES_SYSTEM_REQUIRED;
                if self.state.keep_display_on() {
                    flags |= ES_DISPLAY_REQUIRED;
                }
            }
            self.power_request_succeeded = unsafe { SetThreadExecutionState(flags) }.0 != 0;
            self.last_reapply = Instant::now();
        }

        fn refresh_tray(&mut self, replace_icon: bool) {
            if !self.tray_added {
                return;
            }

            let mut new_icon = None;
            if replace_icon {
                new_icon = create_mug_icon(self.state.enabled(), self.state.is_timed()).ok();
            }
            let icon = new_icon.or(self.icon).unwrap_or_default();
            let data = self.notify_data(icon, self.tooltip());
            unsafe {
                let _ = Shell_NotifyIconW(NIM_MODIFY, &data);
            }
            if let Some(icon) = new_icon {
                if let Some(old) = self.icon.replace(icon) {
                    let _ = unsafe { DestroyIcon(old) };
                }
            }
        }

        fn notify_data(&self, icon: HICON, tooltip: String) -> NOTIFYICONDATAW {
            let mut data = NOTIFYICONDATAW {
                cbSize: size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: self.hwnd,
                uID: TRAY_ID,
                uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
                uCallbackMessage: WM_TRAY,
                hIcon: icon,
                ..Default::default()
            };
            copy_wide(&tooltip, &mut data.szTip);
            data
        }

        fn tooltip(&self) -> String {
            let now = Instant::now();
            match (self.state.enabled(), self.state.remaining(now)) {
                (true, Some(remaining)) => {
                    format!("Brewake — Active for {}", format_duration(remaining))
                }
                (false, Some(remaining)) => {
                    format!("Brewake — Paused for {}", format_duration(remaining))
                }
                (true, None) => "Brewake — Active".into(),
                (false, None) => "Brewake — Inactive".into(),
            }
        }

        fn save_runtime_status(&self) {
            let remaining = self
                .state
                .remaining(Instant::now())
                .map(|value| value.as_secs());
            let _ = save_runtime(&RuntimeStatus {
                running: true,
                enabled: self.state.enabled(),
                keep_display_on: self.state.keep_display_on(),
                power_request_succeeded: self.power_request_succeeded,
                mode: self.state.mode_name().into(),
                remaining_seconds: remaining,
                deadline_unix_seconds: self.deadline_unix_seconds,
            });
        }

        fn tick(&mut self) {
            let now = Instant::now();
            if self.state.tick(now) {
                self.deadline_unix_seconds = None;
                self.apply_and_refresh(true);
                return;
            }

            if self.state.is_timed() {
                self.refresh_tray(false);
            }
            if self.state.enabled()
                && now.duration_since(self.last_reapply) >= Duration::from_secs(50)
            {
                self.apply_power_request();
            }
        }

        fn set_enabled(&mut self, enabled: bool, persist: bool) {
            self.state.set_enabled(enabled);
            self.deadline_unix_seconds = None;
            if persist {
                self.config.enabled = enabled;
                let _ = save_config(&self.config);
            }
            self.apply_and_refresh(true);
        }

        fn toggle(&mut self) {
            self.set_enabled(!self.state.enabled(), true);
        }

        fn activate_for(&mut self, seconds: u64) {
            let seconds = seconds.min(MAX_TIMER_SECONDS);
            if !self
                .state
                .activate_for(Instant::now(), Duration::from_secs(seconds))
            {
                return;
            }
            self.deadline_unix_seconds = unix_now().checked_add(seconds);
            self.apply_and_refresh(true);
        }

        fn pause_for(&mut self, seconds: u64) {
            let seconds = seconds.min(MAX_TIMER_SECONDS);
            if !self
                .state
                .pause_for(Instant::now(), Duration::from_secs(seconds))
            {
                return;
            }
            self.deadline_unix_seconds = unix_now().checked_add(seconds);
            self.apply_and_refresh(true);
        }

        fn toggle_display(&mut self) {
            let enabled = !self.state.keep_display_on();
            self.state.set_keep_display_on(enabled);
            self.config.keep_display_on = enabled;
            let _ = save_config(&self.config);
            self.apply_and_refresh(false);
        }

        fn toggle_startup(&mut self) {
            let enabled = !startup_enabled();
            if set_startup(enabled) {
                self.config.launch_at_startup = enabled;
                let _ = save_config(&self.config);
            }
        }

        fn handle_control(&mut self, command: usize, value: isize) -> UiAction {
            match command {
                CONTROL_ON => self.set_enabled(true, true),
                CONTROL_OFF => self.set_enabled(false, true),
                CONTROL_TOGGLE => self.toggle(),
                CONTROL_ACTIVE_FOR if value > 0 => self.activate_for(value as u64),
                CONTROL_PAUSE_FOR if value > 0 => self.pause_for(value as u64),
                CONTROL_EXIT => return UiAction::Exit(self.hwnd),
                _ => {}
            }
            UiAction::None
        }

        fn menu_snapshot(&self) -> MenuSnapshot {
            MenuSnapshot {
                hwnd: self.hwnd,
                status: self.tooltip().replace("Brewake — ", ""),
                enabled: self.state.enabled(),
                keep_display_on: self.state.keep_display_on(),
                startup_enabled: startup_enabled(),
            }
        }

        fn handle_menu(&mut self, selected: usize) -> UiAction {
            match selected {
                MENU_TOGGLE => self.toggle(),
                MENU_ACTIVE_15M => self.activate_for(15 * 60),
                MENU_ACTIVE_30M => self.activate_for(30 * 60),
                MENU_ACTIVE_1H => self.activate_for(60 * 60),
                MENU_ACTIVE_2H => self.activate_for(2 * 60 * 60),
                MENU_ACTIVE_4H => self.activate_for(4 * 60 * 60),
                MENU_ACTIVE_8H => self.activate_for(8 * 60 * 60),
                MENU_PAUSE_15M => self.pause_for(15 * 60),
                MENU_PAUSE_30M => self.pause_for(30 * 60),
                MENU_PAUSE_1H => self.pause_for(60 * 60),
                MENU_PAUSE_2H => self.pause_for(2 * 60 * 60),
                MENU_PAUSE_4H => self.pause_for(4 * 60 * 60),
                MENU_DISPLAY => self.toggle_display(),
                MENU_STARTUP => self.toggle_startup(),
                MENU_STATUS => {
                    return UiAction::Message {
                        hwnd: self.hwnd,
                        body: self.status_body(),
                        title: "Brewake status",
                    };
                }
                MENU_ABOUT => {
                    return UiAction::Message {
                        hwnd: self.hwnd,
                        body: format!(
                            "Brewake {}\n\nA tiny, native, open-source keep-awake utility for Windows.\n\nNo telemetry. No network access. No administrator rights. No power-plan changes.\n\nGitHub: github.com/thanhbican/brewake\nLicense: MIT",
                            env!("CARGO_PKG_VERSION")
                        ),
                        title: "About Brewake",
                    };
                }
                MENU_EXIT => return UiAction::Exit(self.hwnd),
                _ => {}
            }
            UiAction::None
        }

        fn status_body(&self) -> String {
            format!(
                "{}\n\nComputer sleep: prevented when active\nDisplay sleep: {}\n\nLeft-click the tray icon to toggle.",
                self.tooltip(),
                if self.state.keep_display_on() && self.state.enabled() {
                    "prevented"
                } else {
                    "allowed"
                }
            )
        }

        fn shutdown(&mut self) {
            unsafe {
                SetThreadExecutionState(ES_CONTINUOUS);
            }
            if self.tray_added {
                let data = self.notify_data(self.icon.unwrap_or_default(), String::new());
                unsafe {
                    let _ = Shell_NotifyIconW(NIM_DELETE, &data);
                }
                self.tray_added = false;
            }
            if let Some(icon) = self.icon.take() {
                let _ = unsafe { DestroyIcon(icon) };
            }
            let _ = clear_runtime();
        }
    }

    /// Builds and tracks the native menu from a detached snapshot. This function deliberately
    /// owns no `APP` borrow: `TrackPopupMenuEx` runs a nested message loop, so WM_TIMER and power
    /// messages must be able to access the live state while the menu is open.
    fn show_menu(snapshot: MenuSnapshot) -> usize {
        let Ok(menu) = (unsafe { CreatePopupMenu() }) else {
            return 0;
        };
        let Ok(active_menu) = (unsafe { CreatePopupMenu() }) else {
            let _ = unsafe { DestroyMenu(menu) };
            return 0;
        };
        let Ok(pause_menu) = (unsafe { CreatePopupMenu() }) else {
            let _ = unsafe { DestroyMenu(active_menu) };
            let _ = unsafe { DestroyMenu(menu) };
            return 0;
        };

        append(
            menu,
            MF_STRING | MF_DISABLED | MF_GRAYED,
            0,
            &snapshot.status,
        );
        append(menu, MF_SEPARATOR, 0, "");
        append(
            menu,
            MF_STRING | checked(snapshot.enabled),
            MENU_TOGGLE,
            "Keep awake",
        );

        for (id, label) in [
            (MENU_ACTIVE_15M, "15 minutes"),
            (MENU_ACTIVE_30M, "30 minutes"),
            (MENU_ACTIVE_1H, "1 hour"),
            (MENU_ACTIVE_2H, "2 hours"),
            (MENU_ACTIVE_4H, "4 hours"),
            (MENU_ACTIVE_8H, "8 hours"),
        ] {
            append(active_menu, MF_STRING, id, label);
        }
        append(
            menu,
            MF_STRING | MF_POPUP,
            active_menu.0 as usize,
            "Active for",
        );

        for (id, label) in [
            (MENU_PAUSE_15M, "15 minutes"),
            (MENU_PAUSE_30M, "30 minutes"),
            (MENU_PAUSE_1H, "1 hour"),
            (MENU_PAUSE_2H, "2 hours"),
            (MENU_PAUSE_4H, "4 hours"),
        ] {
            append(pause_menu, MF_STRING, id, label);
        }
        append(
            menu,
            MF_STRING | MF_POPUP,
            pause_menu.0 as usize,
            "Pause for",
        );

        append(menu, MF_SEPARATOR, 0, "");
        append(
            menu,
            MF_STRING | checked(snapshot.keep_display_on),
            MENU_DISPLAY,
            "Keep display on",
        );
        append(
            menu,
            MF_STRING | checked(snapshot.startup_enabled),
            MENU_STARTUP,
            "Start with Windows",
        );
        append(menu, MF_STRING, MENU_STATUS, "Show status");
        append(menu, MF_STRING, MENU_ABOUT, "About Brewake");
        append(menu, MF_SEPARATOR, 0, "");
        append(menu, MF_STRING, MENU_EXIT, "Exit");

        let mut selected = 0;
        let mut point = POINT::default();
        if unsafe { GetCursorPos(&mut point) }.is_ok() {
            unsafe {
                let _ = SetForegroundWindow(snapshot.hwnd);
            }
            let flags = TPM_LEFTALIGN | TPM_BOTTOMALIGN | TPM_RIGHTBUTTON | TPM_RETURNCMD;
            selected = unsafe {
                TrackPopupMenuEx(menu, flags.0, point.x, point.y, snapshot.hwnd, None).0 as usize
            };
            // Required by the notification-area context-menu guidance so the shell dismisses the
            // menu cleanly when the user clicks elsewhere.
            let _ = unsafe { PostMessageW(Some(snapshot.hwnd), WM_NULL, WPARAM(0), LPARAM(0)) };
        }
        let _ = unsafe { DestroyMenu(menu) };
        selected
    }

    fn perform_ui_action(action: UiAction) {
        match action {
            UiAction::None => {}
            UiAction::Message { hwnd, body, title } => message_box(hwnd, &body, title),
            UiAction::Exit(hwnd) => {
                let _ = unsafe { DestroyWindow(hwnd) };
            }
        }
    }

    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        let taskbar_message = TASKBAR_CREATED_MESSAGE.load(Ordering::Relaxed);
        if taskbar_message != 0 && message == taskbar_message {
            APP.with(|slot| {
                if let Some(app) = slot.borrow_mut().as_mut() {
                    app.tray_added = false;
                    let _ = app.add_tray_icon();
                }
            });
            return LRESULT(0);
        }

        match message {
            WM_TRAY => {
                // Version 4 notification callbacks pack the event into LOWORD(lParam); masking is
                // harmless for legacy callbacks and keeps right-click handling future-proof.
                let event = lparam.0 as u32 & 0xFFFF;
                match event {
                    WM_LBUTTONUP => APP.with(|slot| {
                        if let Some(app) = slot.borrow_mut().as_mut() {
                            app.toggle();
                        }
                    }),
                    WM_RBUTTONUP => {
                        let snapshot =
                            APP.with(|slot| slot.borrow().as_ref().map(App::menu_snapshot));
                        if let Some(snapshot) = snapshot {
                            let selected = show_menu(snapshot);
                            let action = APP.with(|slot| {
                                slot.borrow_mut()
                                    .as_mut()
                                    .map(|app| app.handle_menu(selected))
                                    .unwrap_or(UiAction::None)
                            });
                            perform_ui_action(action);
                        }
                    }
                    _ => {}
                }
                LRESULT(0)
            }
            WM_CONTROL => {
                let action = APP.with(|slot| {
                    slot.borrow_mut()
                        .as_mut()
                        .map(|app| app.handle_control(wparam.0, lparam.0))
                        .unwrap_or(UiAction::None)
                });
                perform_ui_action(action);
                LRESULT(0)
            }
            WM_TIMER if wparam.0 == TIMER_ID => {
                APP.with(|slot| {
                    if let Some(app) = slot.borrow_mut().as_mut() {
                        app.tick();
                    }
                });
                LRESULT(0)
            }
            WM_POWERBROADCAST
                if wparam.0 == PBT_APMRESUMEAUTOMATIC as usize
                    || wparam.0 == PBT_APMPOWERSTATUSCHANGE as usize =>
            {
                APP.with(|slot| {
                    if let Some(app) = slot.borrow_mut().as_mut() {
                        app.apply_power_request();
                    }
                });
                LRESULT(1)
            }
            WM_CLOSE => {
                let _ = unsafe { DestroyWindow(hwnd) };
                LRESULT(0)
            }
            WM_DESTROY => {
                unsafe { PostQuitMessage(0) };
                LRESULT(0)
            }
            _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
        }
    }

    fn checked(value: bool) -> MENU_ITEM_FLAGS {
        if value {
            MF_CHECKED
        } else {
            MENU_ITEM_FLAGS(0)
        }
    }

    fn append(menu: HMENU, flags: MENU_ITEM_FLAGS, id: usize, label: &str) {
        let wide = to_wide(label);
        let _ = unsafe { AppendMenuW(menu, flags, id, PCWSTR(wide.as_ptr())) };
    }

    fn message_box(hwnd: HWND, body: &str, title: &str) {
        let body = to_wide(body);
        let title = to_wide(title);
        unsafe {
            MessageBoxW(
                Some(hwnd),
                PCWSTR(body.as_ptr()),
                PCWSTR(title.as_ptr()),
                MB_OK | MB_ICONINFORMATION,
            );
        }
    }

    fn format_duration(duration: Duration) -> String {
        let total_minutes = duration.as_secs().div_ceil(60);
        if total_minutes < 60 {
            format!("{total_minutes}m")
        } else {
            let hours = total_minutes / 60;
            let minutes = total_minutes % 60;
            if minutes == 0 {
                format!("{hours}h")
            } else {
                format!("{hours}h {minutes}m")
            }
        }
    }

    fn unix_now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    fn startup_enabled() -> bool {
        let Some(key) = open_run_key(KEY_QUERY_VALUE) else {
            return false;
        };
        let result = unsafe {
            RegQueryValueExW(key, w!("Brewake"), None, None, None, None) == ERROR_SUCCESS
        };
        unsafe {
            let _ = RegCloseKey(key);
        }
        result
    }

    fn set_startup(enabled: bool) -> bool {
        let Some(key) = open_run_key(KEY_SET_VALUE) else {
            return false;
        };
        let result = if enabled {
            let Ok(executable) = std::env::current_exe() else {
                unsafe {
                    let _ = RegCloseKey(key);
                }
                return false;
            };
            let command = quoted_path(&executable);
            let wide = to_wide(&command);
            let bytes = unsafe {
                slice::from_raw_parts(wide.as_ptr().cast::<u8>(), wide.len() * size_of::<u16>())
            };
            unsafe {
                RegSetValueExW(key, w!("Brewake"), None, REG_SZ, Some(bytes)) == ERROR_SUCCESS
            }
        } else {
            unsafe { RegDeleteValueW(key, w!("Brewake")) == ERROR_SUCCESS }
        };
        unsafe {
            let _ = RegCloseKey(key);
        }
        result
    }

    fn open_run_key(access: windows::Win32::System::Registry::REG_SAM_FLAGS) -> Option<HKEY> {
        let mut key = HKEY::default();
        let result = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, RUN_KEY, None, access, &mut key) };
        (result == ERROR_SUCCESS).then_some(key)
    }

    fn quoted_path(path: &Path) -> String {
        format!("\"{}\"", path.display())
    }

    fn copy_wide(text: &str, destination: &mut [u16]) {
        let maximum = destination.len().saturating_sub(1);
        destination.fill(0);
        for (target, source) in destination
            .iter_mut()
            .zip(text.encode_utf16().take(maximum))
        {
            *target = source;
        }
    }

    fn to_wide(text: &str) -> Vec<u16> {
        std::ffi::OsStr::new(text)
            .encode_wide()
            .chain(Some(0))
            .collect()
    }

    /// Builds a crisp 32 px coffee-mug icon as a standard Windows icon resource.
    /// The drawing is generated in code so source builds do not depend on binary assets.
    fn create_mug_icon(active: bool, timed: bool) -> WinResult<HICON> {
        const SIZE: usize = 32;
        let mut pixels = vec![[0u8; 4]; SIZE * SIZE];
        let outline = if active {
            [77, 43, 20, 255]
        } else {
            [88, 88, 88, 255]
        };
        let cup = if active {
            [230, 132, 44, 255]
        } else {
            [150, 150, 150, 255]
        };
        let coffee = if active {
            [104, 52, 20, 255]
        } else {
            [82, 82, 82, 255]
        };
        let steam = if active {
            [240, 238, 220, 255]
        } else {
            [155, 155, 155, 255]
        };

        // Steam.
        draw_line(&mut pixels, SIZE, 11, 7, 10, 5, steam);
        draw_line(&mut pixels, SIZE, 10, 5, 12, 2, steam);
        draw_line(&mut pixels, SIZE, 18, 7, 17, 5, steam);
        draw_line(&mut pixels, SIZE, 17, 5, 19, 2, steam);

        // Mug body and rim.
        fill_rect(&mut pixels, SIZE, 6, 9, 24, 26, outline);
        fill_rect(&mut pixels, SIZE, 8, 11, 22, 24, cup);
        fill_rect(&mut pixels, SIZE, 8, 11, 22, 14, [244, 242, 226, 255]);
        fill_rect(&mut pixels, SIZE, 9, 13, 21, 14, coffee);
        fill_rect(&mut pixels, SIZE, 8, 15, 22, 23, cup);
        fill_rect(&mut pixels, SIZE, 9, 15, 21, 17, coffee);

        // Handle, with a transparent center.
        fill_rect(&mut pixels, SIZE, 23, 12, 29, 22, outline);
        fill_rect(&mut pixels, SIZE, 24, 14, 27, 20, cup);
        fill_rect(&mut pixels, SIZE, 24, 15, 26, 19, [0, 0, 0, 0]);
        fill_rect(&mut pixels, SIZE, 5, 26, 25, 28, outline);

        // A small clock dot distinguishes a timed session in the crowded tray.
        if timed {
            fill_rect(&mut pixels, SIZE, 23, 3, 30, 10, [28, 107, 82, 255]);
            fill_rect(&mut pixels, SIZE, 25, 4, 27, 7, [245, 245, 235, 255]);
            fill_rect(&mut pixels, SIZE, 27, 6, 29, 7, [245, 245, 235, 255]);
        }

        let resource = icon_resource(&pixels, SIZE);
        unsafe { CreateIconFromResourceEx(&resource, true, 0x0003_0000, 32, 32, LR_DEFAULTCOLOR) }
    }

    fn fill_rect(
        pixels: &mut [[u8; 4]],
        width: usize,
        left: usize,
        top: usize,
        right: usize,
        bottom: usize,
        color: [u8; 4],
    ) {
        for y in top..=bottom {
            for x in left..=right {
                pixels[y * width + x] = color;
            }
        }
    }

    fn draw_line(
        pixels: &mut [[u8; 4]],
        width: usize,
        x0: usize,
        y0: usize,
        x1: usize,
        y1: usize,
        color: [u8; 4],
    ) {
        let steps = x0.abs_diff(x1).max(y0.abs_diff(y1)).max(1);
        for step in 0..=steps {
            let x = (x0 as isize + (x1 as isize - x0 as isize) * step as isize / steps as isize)
                as usize;
            let y = (y0 as isize + (y1 as isize - y0 as isize) * step as isize / steps as isize)
                as usize;
            fill_rect(pixels, width, x, y, x + 1, y + 1, color);
        }
    }

    fn icon_resource(pixels: &[[u8; 4]], size: usize) -> Vec<u8> {
        let color_bytes = size * size * 4;
        let mask_stride = size.div_ceil(32) * 4;
        let mask_bytes = mask_stride * size;
        let mut bytes = Vec::with_capacity(40 + color_bytes + mask_bytes);

        // BITMAPINFOHEADER. ICO stores color and mask heights together.
        push_u32(&mut bytes, 40);
        push_i32(&mut bytes, size as i32);
        push_i32(&mut bytes, (size * 2) as i32);
        push_u16(&mut bytes, 1);
        push_u16(&mut bytes, 32);
        push_u32(&mut bytes, 0);
        push_u32(&mut bytes, color_bytes as u32);
        push_i32(&mut bytes, 0);
        push_i32(&mut bytes, 0);
        push_u32(&mut bytes, 0);
        push_u32(&mut bytes, 0);

        // BGRA, bottom-up. Fully opaque colors avoid shell-dependent alpha blending quirks.
        for y in (0..size).rev() {
            for x in 0..size {
                let [red, green, blue, alpha] = pixels[y * size + x];
                bytes.extend_from_slice(&[blue, green, red, alpha]);
            }
        }

        // AND mask: a set bit means transparent.
        for y in (0..size).rev() {
            let mut row = vec![0u8; mask_stride];
            for x in 0..size {
                if pixels[y * size + x][3] == 0 {
                    row[x / 8] |= 0x80 >> (x % 8);
                }
            }
            bytes.extend_from_slice(&row);
        }
        bytes
    }

    fn push_u16(bytes: &mut Vec<u8>, value: u16) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn push_u32(bytes: &mut Vec<u8>, value: u32) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn push_i32(bytes: &mut Vec<u8>, value: i32) {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
}
