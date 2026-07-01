use std::env;
use std::process::{Command, ExitCode};

use korean_core::{HangulComposer, InputResult};

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("setup") => setup(),
        Some("status") => status(),
        Some("doctor") => doctor(),
        Some("simulate") => {
            let input = args.next().unwrap_or_default();
            simulate(&input)
        }
        Some("reset") => reset(),
        _ => {
            print_usage();
            ExitCode::from(2)
        }
    }
}

fn print_usage() {
    eprintln!(
        "Usage:
  korean setup
  korean status
  korean doctor
  korean simulate <keys>
  korean reset"
    );
}

fn setup() -> ExitCode {
    let engine = engine_name();
    if !command_exists("ibus") {
        eprintln!("ibus command not found. Install the korean package dependencies and try again.");
        return ExitCode::from(1);
    }

    let _ = Command::new("ibus").arg("start").status();

    if !engine_registered(&engine) {
        eprintln!("{engine} engine is not visible to IBus yet.");
        eprintln!("Try: ibus restart");
        eprintln!("If it still fails, verify the IBus component XML exists.");
        return ExitCode::from(1);
    }

    if command_exists("gsettings") {
        let mut current = gsettings("get", "org.gnome.desktop.input-sources", "sources")
            .unwrap_or_else(|| "[]".to_string());
        if !current.contains(&engine) {
            let updated = append_ibus_source(&current, &engine);
            if !run_gsettings_set("sources", &updated) {
                eprintln!("Could not update GNOME input sources automatically.");
                eprintln!("Add ('ibus', '{engine}') from Settings > Keyboard > Input Sources.");
                return ExitCode::from(1);
            }
            current = updated;
        }
        if let Some(index) = source_index(&current, &engine) {
            if !run_gsettings_set("current", &index.to_string()) {
                eprintln!("Could not select {engine} automatically.");
                eprintln!("Select '{engine}' from the GNOME input source menu.");
                return ExitCode::from(1);
            }
        } else {
            eprintln!("{engine} was not found in GNOME input sources after setup.");
            eprintln!("Add ('ibus', '{engine}') from Settings > Keyboard > Input Sources.");
            return ExitCode::from(1);
        }
    } else {
        eprintln!("gsettings not found. Add Korean manually in GNOME Settings.");
    }

    println!("Korean setup completed.");
    println!("Select '{engine}' in the GNOME input source menu if it is not active yet.");
    ExitCode::SUCCESS
}

fn status() -> ExitCode {
    let engine = engine_name();
    println!("Korean status");
    println!(
        "  ibus: {}",
        if command_exists("ibus") {
            "found"
        } else {
            "missing"
        }
    );
    println!(
        "  engine: {}",
        if engine_registered(&engine) {
            "registered"
        } else {
            "not registered"
        }
    );
    println!(
        "  running ibus visibility: {}",
        if engine_visible_to_running_ibus(&engine) {
            "visible"
        } else {
            "not visible"
        }
    );
    if let Some(sources) = gsettings("get", "org.gnome.desktop.input-sources", "sources") {
        println!("  gnome sources: {sources}");
    }
    ExitCode::SUCCESS
}

fn doctor() -> ExitCode {
    let engine = engine_name();
    let mut ok = true;
    for command in ["ibus", "gsettings"] {
        let found = command_exists(command);
        println!("{command}: {}", if found { "ok" } else { "missing" });
        ok &= found;
    }

    if std::env::var("IBUS_ADDRESS").is_ok() {
        println!("IBUS_ADDRESS: set");
    } else {
        println!("IBUS_ADDRESS: not set");
        println!("  In GNOME, this is usually provided to graphical apps by the session.");
        println!("  If the engine cannot connect, try: ibus restart");
    }

    if !engine_registered(&engine) {
        ok = false;
        println!("{engine} engine: not registered");
        println!("  Expected an IBus component XML for {engine}");
        println!("  Try: sudo apt install --reinstall korean && ibus restart");
    } else {
        println!("{engine} engine: ok");
        if !engine_visible_to_running_ibus(&engine) {
            println!("  Installed component is present, but the current shell cannot query the running IBus engine list.");
            println!("  If GNOME does not show Korean, run: ibus restart");
        }
    }

    if !std::path::Path::new("/dev/input").exists() {
        println!("caps daemon: /dev/input is not available in this environment");
    } else {
        println!("caps daemon: /dev/input exists");
        println!(
            "  If tap/hold fails, ensure your user can read the relevant /dev/input/event* device."
        );
    }

    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn simulate(input: &str) -> ExitCode {
    let mut composer = HangulComposer::new();
    let mut committed = String::new();
    for ch in input.chars() {
        match composer.input_key(ch) {
            InputResult::Commit { text } => committed.push_str(&text),
            InputResult::CommitAndPreedit { commit, .. } => committed.push_str(&commit),
            _ => {}
        }
        println!("{ch} -> {committed}{}", composer.preedit());
    }
    if let Some(text) = composer.commit() {
        committed.push_str(&text);
    }
    println!("final: {committed}");
    ExitCode::SUCCESS
}

fn reset() -> ExitCode {
    println!("Korean reset requested.");
    println!("Current MVP keeps composition state inside the active IBus engine process.");
    ExitCode::SUCCESS
}

fn command_exists(name: &str) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {name} >/dev/null 2>&1"))
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn engine_name() -> String {
    std::env::var("KOREAN_ENGINE").unwrap_or_else(|_| "korean".to_string())
}

fn engine_registered(engine: &str) -> bool {
    ibus_output_contains(["list-engine"], engine)
        || ibus_output_contains(["read-cache"], engine)
        || component_file_exists(engine)
}

fn ibus_output_contains<const N: usize>(args: [&str; N], needle: &str) -> bool {
    Command::new("ibus")
        .args(args)
        .output()
        .map(|out| out.status.success() && String::from_utf8_lossy(&out.stdout).contains(needle))
        .unwrap_or(false)
}

fn engine_visible_to_running_ibus(engine: &str) -> bool {
    Command::new("ibus")
        .arg("list-engine")
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).contains(engine))
        .unwrap_or(false)
}

fn component_file_exists(engine: &str) -> bool {
    let file_name = format!("{engine}.xml");
    if std::path::Path::new("/usr/share/ibus/component")
        .join(&file_name)
        .exists()
    {
        return true;
    }

    let Some(home) = std::env::var_os("HOME") else {
        return false;
    };
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(home).join(".local/share"));
    data_home.join("ibus/component").join(file_name).exists()
}

fn gsettings(action: &str, schema: &str, key: &str) -> Option<String> {
    let out = Command::new("gsettings")
        .arg(action)
        .arg(schema)
        .arg(key)
        .output()
        .ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        None
    }
}

fn run_gsettings_set(key: &str, value: &str) -> bool {
    Command::new("gsettings")
        .arg("set")
        .arg("org.gnome.desktop.input-sources")
        .arg(key)
        .arg(value)
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn append_ibus_source(current: &str, engine: &str) -> String {
    let item = format!("('ibus', '{engine}')");
    let trimmed = current.trim();
    if trimmed == "[]" || trimmed.is_empty() {
        format!("[{item}]")
    } else if let Some(prefix) = trimmed.strip_suffix(']') {
        format!("{}, {item}]", prefix.trim_end())
    } else {
        format!("[{item}]")
    }
}

fn source_index(current: &str, engine: &str) -> Option<usize> {
    let needle = format!("('ibus', '{engine}')");
    let mut index = 0;
    for part in current.split("),") {
        let normalized = if part.trim_end().ends_with(')') {
            part.trim().to_string()
        } else {
            format!("{})", part.trim())
        };
        if normalized.contains(&needle) {
            return Some(index);
        }
        if normalized.contains("('") {
            index += 1;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{append_ibus_source, source_index};

    #[test]
    fn appends_korean_to_empty_sources() {
        assert_eq!(append_ibus_source("[]", "korean"), "[('ibus', 'korean')]");
    }

    #[test]
    fn finds_korean_source_index() {
        let sources = "[('xkb', 'us'), ('ibus', 'korean')]";
        assert_eq!(source_index(sources, "korean"), Some(1));
    }

    #[test]
    fn finds_korean_when_it_is_first() {
        let sources = "[('ibus', 'korean'), ('xkb', 'us')]";
        assert_eq!(source_index(sources, "korean"), Some(0));
    }
}
