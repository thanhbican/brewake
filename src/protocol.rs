//! The tiny IPC protocol used between `brewakectl` and the hidden tray window.

pub const WINDOW_CLASS: &str = "Brewake.TrayWindow.v1";
pub const SINGLE_INSTANCE_MUTEX: &str = "Local\\Brewake.SingleInstance.v1";
pub const MAX_TIMER_SECONDS: u64 = 365 * 24 * 60 * 60;

pub const WM_TRAY: u32 = 0x8000 + 1;
pub const WM_CONTROL: u32 = 0x8000 + 2;

pub const CONTROL_ON: usize = 1;
pub const CONTROL_OFF: usize = 2;
pub const CONTROL_TOGGLE: usize = 3;
pub const CONTROL_ACTIVE_FOR: usize = 4;
pub const CONTROL_PAUSE_FOR: usize = 5;
pub const CONTROL_EXIT: usize = 6;
