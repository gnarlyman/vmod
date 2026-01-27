use std::fs;
use std::path::PathBuf;

/// Initialize the logging system with dual output to stderr and file.
///
/// Log file is written to `~/.config/vmod/vmod.log`
/// Debug level in debug builds, Info level in release builds.
pub fn init() -> Result<(), fern::InitError> {
    let log_level = if cfg!(debug_assertions) {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Info
    };

    let mut dispatch = fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{}] [{}] {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                message
            ))
        })
        .level(log_level)
        .chain(std::io::stderr());

    // Add file logging if we can create the log file
    if let Some(log_path) = get_log_path() {
        // Ensure parent directory exists
        if let Some(parent) = log_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        if let Ok(log_file) = fern::log_file(&log_path) {
            dispatch = dispatch.chain(log_file);
        }
    }

    dispatch.apply()?;
    Ok(())
}

/// Get the path for the log file
fn get_log_path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("vmod").join("vmod.log"))
}
