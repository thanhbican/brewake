# Architecture

Brewake intentionally stays Windows-first and dependency-light.

```text
brewake.exe
  ├─ hidden Win32 window + message loop
  ├─ Shell_NotifyIcon tray icon and native popup menus
  ├─ AwakeState timer/state machine
  ├─ SetThreadExecutionState power request
  ├─ HKCU startup entry
  └─ JSON config/runtime snapshots

brewakectl.exe
  ├─ finds the hidden Brewake window
  ├─ sends a private WM_APP control message
  └─ reads the runtime snapshot for status output
```

## Design decisions

- **No simulated key presses.** They can leak into documents, remote sessions, presentations, and games. Windows already exposes a purpose-built power API.
- **No service or elevation.** The execution-state request belongs to the process and is automatically released if it exits.
- **No Tauri/WebView.** A tray-only utility needs a message loop and native menu, not a browser runtime.
- **Two executables.** `brewake.exe` uses the Windows GUI subsystem so it never flashes a console. `brewakectl.exe` remains a normal console program with script-friendly output.
- **Platform-neutral state machine.** Timer transitions and persistence models are testable without creating a window.

## State transitions

```text
Inactive ── on/toggle ──> Active
Active   ── off/toggle ─> Inactive
Either   ── on --for ───> Active (timed) ── deadline ─> Inactive
Either   ── pause ──────> Paused (timed) ── deadline ─> Active
```

The app re-applies an active request every 50 seconds and after relevant power broadcasts. This is deliberately defensive: the request is continuous, but suspend/resume and shell lifecycle edges should not surprise users.

## Persistence

`%LocalAppData%\Brewake\config.json` stores durable preferences. `runtime.json` is only a CLI-readable snapshot and is removed during a clean exit. Neither file contains personal data.
