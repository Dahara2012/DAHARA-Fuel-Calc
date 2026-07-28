use serde::Deserialize;
use std::path::PathBuf;
use std::process::{ExitStatus, Stdio};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

fn exit_status_desc(status: &ExitStatus) -> String {
    let code = format!("{:?}", status.code());
    #[cfg(unix)]
    { format!("code={code} signal={:?}", status.signal()) }
    #[cfg(not(unix))]
    { format!("code={code}") }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum SidecarEvent {
    #[serde(rename = "state")]
    FuelState(serde_json::Value),
    #[serde(rename = "session-info")]
    SessionInfo(serde_json::Value),
    #[serde(rename = "status")]
    Status(serde_json::Value),
}

pub fn resolve_sidecar_path(app: &AppHandle) -> Result<PathBuf, String> {
    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|e| format!("resource_dir: {e}"))?;

    let exe_name = if cfg!(windows) {
        "dahara-fuel-calc-sidecar-x86_64-pc-windows-msvc.exe"
    } else {
        "dahara-fuel-calc-sidecar"
    };

    let mut candidates: Vec<PathBuf> = Vec::new();
    candidates.push(resource_dir.join("binaries").join(exe_name));
    candidates.push(resource_dir.join(exe_name));
    if let Some(parent) = resource_dir.parent() {
        candidates.push(parent.join(exe_name));
    }
    candidates.push(resource_dir.join("resources").join("binaries").join(exe_name));
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("binaries").join(exe_name));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            candidates.push(parent.join(exe_name));
        }
    }

    for c in &candidates {
        if c.exists() {
            return Ok(c.clone());
        }
    }

    Err(format!(
        "sidecar binary not found. resource_dir={:?} tried: {:?}",
        resource_dir, candidates
    ))
}

pub async fn spawn_and_pump(app: AppHandle) -> Result<(), String> {
    let sidecar_path = resolve_sidecar_path(&app)?;

    let mut cmd = Command::new(&sidecar_path);
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .stdin(Stdio::null())
        .kill_on_drop(true);

    if let Some(parent) = sidecar_path.parent() {
        cmd.current_dir(parent);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("spawn sidecar: {e}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "sidecar stdout missing".to_string())?;

    let mut reader = BufReader::new(stdout).lines();
    let app_clone = app.clone();

    tokio::spawn(async move {
        while let Ok(Some(line)) = reader.next_line().await {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            // Parse into a generic JSON value and re-emit as-is, so the
            // `type` discriminator field survives the IPC hop and the
            // renderer can use `isFuelState()` / `inRace` checks.
            match serde_json::from_str::<serde_json::Value>(line) {
                Ok(payload) => {
                    if let Err(e) = app_clone.emit("fuel", payload) {
                        eprintln!("[sidecar] emit error: {e}");
                    }
                }
                Err(err) => {
                    eprintln!("[sidecar] json parse error: {err} line={line}");
                }
            }
        }

        match child.wait().await {
            Ok(status) => eprintln!(
                "[sidecar] process exited: {}",
                exit_status_desc(&status)
            ),
            Err(err) => eprintln!("[sidecar] wait error: {err}"),
        }
    });

    Ok(())
}
