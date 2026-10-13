//! Reload the config when its file (or its directory, for includes) changes.

use std::path::{Path, PathBuf};
use std::time::Duration;

use notify::{RecursiveMode, Watcher};
use smithay::reexports::calloop::LoopHandle;
use smithay::reexports::calloop::channel::{Event, channel};
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};

use crate::state::State;

/// Keeps the watcher alive.
pub struct ConfigWatcher {
    _watcher: notify::RecommendedWatcher,
}

pub fn watch(path: &Path, event_loop: &LoopHandle<'static, State>) -> anyhow::Result<ConfigWatcher> {
    let dir = path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    std::fs::create_dir_all(&dir).ok();
    let (tx, rx) = channel::<()>();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res
            && (event.kind.is_modify() || event.kind.is_create() || event.kind.is_remove())
            && event
                .paths
                .iter()
                .any(|p| p.extension().is_some_and(|e| e == "toml" || e == "json"))
        {
            let _ = tx.send(());
        }
    })?;
    watcher.watch(&dir, RecursiveMode::NonRecursive)?;

    let handle = event_loop.clone();
    event_loop
        .insert_source(rx, move |event, _, state| {
            if let Event::Msg(()) = event
                && !state.hecate.reload_pending
            {
                // Editors write in several steps; wait for things to settle.
                state.hecate.reload_pending = true;
                let _ =
                    handle.insert_source(Timer::from_duration(Duration::from_millis(150)), |_, _, state| {
                        state.hecate.reload_pending = false;
                        state.reload_config();
                        TimeoutAction::Drop
                    });
            }
        })
        .map_err(|e| anyhow::anyhow!("inserting config watcher: {e}"))?;
    Ok(ConfigWatcher { _watcher: watcher })
}
