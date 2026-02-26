use anyhow::Error;
use auto_launch::{AutoLaunch, AutoLaunchBuilder, LinuxLaunchMode, MacOSLaunchMode};
use std::{env::args, fs::canonicalize};

// Create auto launch profile with auto_launch library
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

    Ok(AutoLaunchBuilder::new()
        .set_app_name("Kucheat")
        .set_app_path(path)
        .set_args(&["--daemon"])
        .set_linux_launch_mode(LinuxLaunchMode::XdgAutostart)
        .set_macos_launch_mode(MacOSLaunchMode::LaunchAgent)
        .build()?)
}
