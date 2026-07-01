use std::env;
use std::process::{Command, ExitCode};

use korean_core::{HangulComposer, InputResult};

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("start") => start(args.collect()),
        Some("stop") => stop(),
        Some("setup") => setup(args.collect()),
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
  korean start [--quiet]
  korean stop
  korean setup [--caps-switch] [--quiet]
  korean status
  korean doctor
  korean simulate <keys>
  korean reset"
    );
}

fn start(args: Vec<String>) -> ExitCode {
    setup(args)
}

fn stop() -> ExitCode {
    let engine = engine_name();
    if !command_exists("gsettings") {
        eprintln!("gsettings not found. Remove Korean manually in GNOME Settings.");
        return ExitCode::from(1);
    }

    let current = gsettings("get", "org.gnome.desktop.input-sources", "sources")
        .unwrap_or_else(|| "[]".to_string());
    let updated = remove_ibus_source(&current, &engine);
    if updated != current && !run_gsettings_set("sources", &updated) {
        eprintln!("Could not remove {engine} from GNOME input sources automatically.");
        return ExitCode::from(1);
    }

    if updated != "[]" {
        let _ = run_gsettings_set("current", "0");
    }

    if !restore_default_switch_keys() {
        eprintln!("Could not restore GNOME input-source switch keys automatically.");
        return ExitCode::from(1);
    }

    println!("Korean stopped.");
    ExitCode::SUCCESS
}

#[derive(Default)]
struct SetupOptions {
    caps_switch: bool,
    quiet: bool,
}

fn setup(args: Vec<String>) -> ExitCode {
    let options = match parse_setup_options(args) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}");
            print_usage();
            return ExitCode::from(2);
        }
    };
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

        if options.caps_switch && !configure_caps_switch() {
            eprintln!("Could not set Caps Lock as the GNOME input-source switch key.");
            eprintln!("Set Settings > Keyboard > Keyboard Shortcuts > Typing > Switch to next input source to Caps Lock.");
            return ExitCode::from(1);
        }

        if !options.caps_switch && !restore_default_switch_keys() {
            eprintln!("Could not restore GNOME input-source switch keys automatically.");
            return ExitCode::from(1);
        }
    } else {
        eprintln!("gsettings not found. Add Korean manually in GNOME Settings.");
    }

    if !restart_ibus() {
        eprintln!("Could not restart IBus automatically.");
        eprintln!("Run: ibus restart");
        return ExitCode::from(1);
    }

    if !options.quiet {
        println!("Korean setup completed.");
        println!("IBus restarted.");
        if options.caps_switch {
            println!("Caps Lock is configured as the GNOME input source switch key.");
        } else {
            println!("Caps Lock is handled by the Korean input method.");
        }
        println!("Select '{engine}' in the GNOME input source menu if it is not active yet.");
    }
    ExitCode::SUCCESS
}

fn parse_setup_options(args: Vec<String>) -> Result<SetupOptions, String> {
    let mut options = SetupOptions::default();
    for arg in args {
        match arg.as_str() {
            "--caps-switch" => options.caps_switch = true,
            "--quiet" => options.quiet = true,
            _ => return Err(format!("Unknown setup option: {arg}")),
        }
    }
    Ok(options)
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
    run_gsettings_set_schema("org.gnome.desktop.input-sources", key, value)
}

fn run_gsettings_set_schema(schema: &str, key: &str, value: &str) -> bool {
    Command::new("gsettings")
        .arg("set")
        .arg(schema)
        .arg(key)
        .arg(value)
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn restart_ibus() -> bool {
    Command::new("ibus")
        .arg("restart")
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn configure_caps_switch() -> bool {
    run_gsettings_set_schema(
        "org.gnome.desktop.wm.keybindings",
        "switch-input-source",
        "['Caps_Lock']",
    ) && run_gsettings_set_schema(
        "org.gnome.desktop.wm.keybindings",
        "switch-input-source-backward",
        "[]",
    )
}

fn restore_default_switch_keys() -> bool {
    run_gsettings_set_schema(
        "org.gnome.desktop.wm.keybindings",
        "switch-input-source",
        "['<Super>space']",
    ) && run_gsettings_set_schema(
        "org.gnome.desktop.wm.keybindings",
        "switch-input-source-backward",
        "['<Shift><Super>space']",
    )
}

fn append_ibus_source(current: &str, engine: &str) -> String {
    let item = format!("('ibus', '{engine}')");
    let mut items = source_items(current);
    if items.iter().any(|source| source == &item) {
        source_list(&items)
    } else {
        items.push(item);
        source_list(&items)
    }
}

fn source_list(items: &[String]) -> String {
    if items.is_empty() {
        "[]".to_string()
    } else {
        format!("[{}]", items.join(", "))
    }
}

fn remove_ibus_source(current: &str, engine: &str) -> String {
    let needle = format!("('ibus', '{engine}')");
    let items = source_items(current)
        .into_iter()
        .filter(|item| !item.contains(&needle))
        .collect::<Vec<_>>();
    source_list(&items)
}

fn source_index(current: &str, engine: &str) -> Option<usize> {
    let needle = format!("('ibus', '{engine}')");
    for (index, item) in source_items(current).iter().enumerate() {
        if item.contains(&needle) {
            return Some(index);
        }
    }
    None
}

fn source_items(current: &str) -> Vec<String> {
    let trimmed = current.trim();
    let without_type = trimmed.strip_prefix("@a(ss)").unwrap_or(trimmed).trim();
    let inner = without_type
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or(without_type)
        .trim();
    if inner.is_empty() {
        return Vec::new();
    }

    inner
        .split("),")
        .filter_map(|part| {
            let part = part.trim();
            if part.is_empty() {
                None
            } else if part.ends_with(')') {
                Some(part.to_string())
            } else {
                Some(format!("{part})"))
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{append_ibus_source, parse_setup_options, remove_ibus_source, source_index};

    #[test]
    fn appends_korean_to_empty_sources() {
        assert_eq!(append_ibus_source("[]", "korean"), "[('ibus', 'korean')]");
    }

    #[test]
    fn appends_korean_to_typed_empty_sources() {
        assert_eq!(
            append_ibus_source("@a(ss) []", "korean"),
            "[('ibus', 'korean')]"
        );
    }

    #[test]
    fn does_not_duplicate_existing_korean_source() {
        let sources = "[('ibus', 'korean')]";
        assert_eq!(append_ibus_source(sources, "korean"), sources);
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

    #[test]
    fn removes_korean_source() {
        let sources = "[('xkb', 'us'), ('ibus', 'korean'), ('ibus', 'hangul')]";
        assert_eq!(
            remove_ibus_source(sources, "korean"),
            "[('xkb', 'us'), ('ibus', 'hangul')]"
        );
    }

    #[test]
    fn removes_korean_when_it_is_only_source() {
        assert_eq!(remove_ibus_source("[('ibus', 'korean')]", "korean"), "[]");
    }

    #[test]
    fn removes_from_typed_empty_sources() {
        assert_eq!(remove_ibus_source("@a(ss) []", "korean"), "[]");
    }

    #[test]
    fn parses_setup_caps_switch_option() {
        let options = parse_setup_options(vec!["--caps-switch".into(), "--quiet".into()]).unwrap();
        assert!(options.caps_switch);
        assert!(options.quiet);
    }

    #[test]
    fn rejects_unknown_setup_option() {
        assert!(parse_setup_options(vec!["--unknown".into()]).is_err());
    }
}
