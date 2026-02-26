use anyhow::Error;
use auto_launch::{AutoLaunch, LinuxLaunchMode};
use std::{env::args, fs::canonicalize};

pub fn get_auto_launch() -> anyhow::Result<AutoLaunch> {
    // Retrieve the path of current process's binary
    let mut args_iter = args();
    let path = args_iter.next().unwrap();
    let path = match canonicalize(path) {
        Ok(path) => path,
        Err(err) => {
            tracing::error!("Error while enabling auto-launch: {err}");
            tracing::warn!("Note: To enable auto-launch, execute Kucheat with an absolute path");
            return Err(Error::msg(err.to_string()));
        }
    };
    let path = path.to_str().unwrap();

    Ok(AutoLaunch::new(
        "Kucheat",
        path,
        LinuxLaunchMode::XdgAutostart,
        &["daemon"],
    ))
}
