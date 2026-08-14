use brewake::{
    config::{RuntimeStatus, load_runtime},
    protocol::{
        CONTROL_ACTIVE_FOR, CONTROL_EXIT, CONTROL_OFF, CONTROL_ON, CONTROL_PAUSE_FOR,
        CONTROL_TOGGLE, MAX_TIMER_SECONDS,
    },
};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(not(windows))]
fn main() {
    eprintln!("brewakectl currently supports Windows only.");
    std::process::exit(1);
}

#[cfg(windows)]
fn main() {
    match parse_args(std::env::args().skip(1).collect()) {
        Ok(CliCommand::Help) => print_help(),
        Ok(CliCommand::Version) => println!("brewakectl {}", env!("CARGO_PKG_VERSION")),
        Ok(CliCommand::Status { json }) => print_status(json),
        Ok(CliCommand::Control {
            command,
            seconds,
            start_if_missing,
        }) => {
            if let Err(error) = windows_cli::send(command, seconds, start_if_missing) {
                eprintln!("brewakectl: {error}");
                std::process::exit(1);
            }
        }
        Err(error) => {
            eprintln!("brewakectl: {error}\n");
            print_help();
            std::process::exit(2);
        }
    }
}

#[cfg(windows)]
fn print_status(json: bool) {
    let running = windows_cli::is_running();
    let mut status = if running {
        load_runtime()
    } else {
        RuntimeStatus::stopped()
    };
    status.running = running;
    if let Some(deadline) = status.deadline_unix_seconds {
        status.remaining_seconds = Some(deadline.saturating_sub(unix_now()));
    }

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&status).expect("runtime status is serializable")
        );
        return;
    }

    if !status.running {
        println!("Brewake is not running.");
        return;
    }
    let mode = match status.mode.as_str() {
        "active_for" => format!(
            "active for {}",
            format_duration(status.remaining_seconds.unwrap_or(0))
        ),
        "paused_for" => format!(
            "paused for {}",
            format_duration(status.remaining_seconds.unwrap_or(0))
        ),
        "active" => "active".into(),
        _ => "inactive".into(),
    };
    println!("Brewake is {mode}.");
    if status.enabled {
        println!(
            "System sleep request: {}",
            if status.power_request_succeeded {
                "accepted by Windows"
            } else {
                "FAILED"
            }
        );
    }
    println!(
        "Display sleep: {}",
        if status.enabled && status.keep_display_on {
            "prevented"
        } else {
            "allowed"
        }
    );
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CliCommand {
    Help,
    Version,
    Status {
        json: bool,
    },
    Control {
        command: usize,
        seconds: isize,
        start_if_missing: bool,
    },
}

fn parse_args(args: Vec<String>) -> Result<CliCommand, String> {
    let Some(command) = args.first().map(String::as_str) else {
        return Ok(CliCommand::Status { json: false });
    };
    match command {
        "help" | "--help" | "-h" => Ok(CliCommand::Help),
        "version" | "--version" | "-V" => Ok(CliCommand::Version),
        "status" => match args.get(1).map(String::as_str) {
            None => Ok(CliCommand::Status { json: false }),
            Some("--json") => Ok(CliCommand::Status { json: true }),
            Some(value) => Err(format!("unknown status option: {value}")),
        },
        "on" | "active" => {
            if args.len() == 1 {
                control(CONTROL_ON, 0, true)
            } else if args.len() == 3 && args[1] == "--for" {
                control(CONTROL_ACTIVE_FOR, parse_duration(&args[2])? as isize, true)
            } else {
                Err("usage: brewakectl on [--for 15m|2h|90s]".into())
            }
        }
        "off" | "inactive" => exact_control(&args, CONTROL_OFF, true),
        "toggle" => exact_control(&args, CONTROL_TOGGLE, true),
        "pause" => {
            if args.len() != 2 {
                return Err("usage: brewakectl pause 15m|2h|90s".into());
            }
            control(CONTROL_PAUSE_FOR, parse_duration(&args[1])? as isize, true)
        }
        "exit" | "quit" => exact_control(&args, CONTROL_EXIT, false),
        value => Err(format!("unknown command: {value}")),
    }
}

fn exact_control(
    args: &[String],
    command: usize,
    start_if_missing: bool,
) -> Result<CliCommand, String> {
    if args.len() != 1 {
        return Err(format!("{} does not accept arguments", args[0]));
    }
    control(command, 0, start_if_missing)
}

fn control(command: usize, seconds: isize, start_if_missing: bool) -> Result<CliCommand, String> {
    Ok(CliCommand::Control {
        command,
        seconds,
        start_if_missing,
    })
}

fn parse_duration(value: &str) -> Result<u64, String> {
    let split_at = value
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(value.len());
    let (number, unit) = value.split_at(split_at);
    if number.is_empty() || unit.is_empty() || unit.len() != 1 {
        return Err(format!("invalid duration: {value}; use 15m, 2h, or 90s"));
    }
    let amount: u64 = number
        .parse()
        .map_err(|_| format!("invalid duration: {value}"))?;
    if amount == 0 {
        return Err("duration must be greater than zero".into());
    }
    let multiplier = match unit {
        "s" => 1,
        "m" => 60,
        "h" => 60 * 60,
        _ => return Err(format!("unsupported duration unit: {unit}; use s, m, or h")),
    };
    amount
        .checked_mul(multiplier)
        .filter(|seconds| *seconds <= MAX_TIMER_SECONDS)
        .ok_or_else(|| "duration is too large; maximum is 365 days".into())
}

#[cfg(windows)]
fn print_help() {
    println!(
        "Brewake command-line control\n\n\
         Usage:\n\
           brewakectl status [--json]\n\
           brewakectl on [--for 15m|2h|90s]\n\
           brewakectl off\n\
           brewakectl toggle\n\
           brewakectl pause 15m|2h|90s\n\
           brewakectl exit"
    );
}

fn format_duration(seconds: u64) -> String {
    if seconds < 60 {
        return format!("{seconds}s");
    }
    let minutes = seconds.div_ceil(60);
    if minutes < 60 {
        return format!("{minutes}m");
    }
    let hours = minutes / 60;
    let remainder = minutes % 60;
    if remainder == 0 {
        format!("{hours}h")
    } else {
        format!("{hours}h {remainder}m")
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(windows)]
mod windows_cli {
    use brewake::protocol::WM_CONTROL;
    use std::{thread, time::Duration};
    use windows::{
        Win32::{
            Foundation::{HWND, LPARAM, WPARAM},
            UI::WindowsAndMessaging::{FindWindowW, PostMessageW},
        },
        core::{PCWSTR, w},
    };

    pub fn is_running() -> bool {
        find_window().is_some()
    }

    pub fn send(command: usize, seconds: isize, start_if_missing: bool) -> Result<(), String> {
        let hwnd = match find_window() {
            Some(hwnd) => hwnd,
            None if start_if_missing => start_app()?,
            None => return Err("Brewake is not running".into()),
        };
        unsafe {
            PostMessageW(Some(hwnd), WM_CONTROL, WPARAM(command), LPARAM(seconds))
                .map_err(|error| error.to_string())
        }
    }

    fn find_window() -> Option<HWND> {
        unsafe { FindWindowW(w!("Brewake.TrayWindow.v1"), PCWSTR::null()).ok() }
    }

    fn start_app() -> Result<HWND, String> {
        let mut executable = std::env::current_exe().map_err(|error| error.to_string())?;
        executable.set_file_name("brewake.exe");
        if !executable.is_file() {
            return Err(format!("could not find {}", executable.display()));
        }
        std::process::Command::new(&executable)
            .spawn()
            .map_err(|error| format!("could not start Brewake: {error}"))?;

        for _ in 0..40 {
            if let Some(hwnd) = find_window() {
                return Ok(hwnd);
            }
            thread::sleep(Duration::from_millis(50));
        }
        Err("Brewake did not become ready within two seconds".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_duration_units() {
        assert_eq!(parse_duration("90s").unwrap(), 90);
        assert_eq!(parse_duration("15m").unwrap(), 900);
        assert_eq!(parse_duration("2h").unwrap(), 7_200);
    }

    #[test]
    fn rejects_ambiguous_durations() {
        assert!(parse_duration("15").is_err());
        assert!(parse_duration("1d").is_err());
        assert!(parse_duration("0m").is_err());
        assert!(parse_duration("8761h").is_err());
    }

    #[test]
    fn parses_timed_on_command() {
        assert_eq!(
            parse_args(vec!["on".into(), "--for".into(), "2h".into()]).unwrap(),
            CliCommand::Control {
                command: CONTROL_ACTIVE_FOR,
                seconds: 7_200,
                start_if_missing: true,
            }
        );
    }
}
