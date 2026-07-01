use std::fs;
use std::process::ExitCode;
use std::time::Duration;

fn main() -> ExitCode {
    let events = match fs::read_dir("/dev/input") {
        Ok(entries) => entries
            .flatten()
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|name| name.starts_with("event"))
            .collect::<Vec<_>>(),
        Err(err) => {
            eprintln!("Cannot read /dev/input: {err}");
            eprintln!(
                "Install the korean package, reload udev rules, then log out and log back in."
            );
            return ExitCode::from(1);
        }
    };

    if events.is_empty() {
        eprintln!("No /dev/input/event* devices found.");
        return ExitCode::from(1);
    }

    eprintln!("korean-capsd found {} input event devices.", events.len());
    eprintln!("Caps tap/hold event handling is not enabled in this MVP yet.");
    eprintln!(
        "The daemon is intentionally optional; Korean can still be selected as an IBus source."
    );

    loop {
        std::thread::sleep(Duration::from_secs(3600));
    }
}
