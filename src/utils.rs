use gettextrs::*;
use std::collections::HashMap;
use std::env;
use std::path::Path;
use std::process::Command;

pub fn run_command(
    cmd_to_run: std::string::String,
    args_for_cmd: Option<&[&str]>,
) -> std::result::Result<std::process::Output, std::io::Error> {
    let mut cmd = Command::new(cmd_to_run.clone());

    if is_flatpak() {
        cmd = Command::new("flatpak-spawn");
        cmd.arg("--host");
        cmd.arg(&cmd_to_run);
    }

    if let Some(a) = args_for_cmd {
        cmd.args(a);
    }

    cmd.output()
}

pub fn get_command_output(
    cmd_to_run: std::string::String,
    args_for_cmd: Option<&[&str]>,
) -> std::string::String {
    let output = run_command(cmd_to_run, args_for_cmd);

    match output {
        Ok(o) => {
            let mut result = String::from("");
            if !o.stdout.is_empty() {
                result = result
                    + String::from_utf8_lossy(&o.stdout).into_owned().as_ref()
                    + &String::from("\n");
            }

            if !o.stderr.is_empty() {
                result = result
                    + String::from_utf8_lossy(&o.stderr).into_owned().as_ref()
                    + &String::from("\n");
            }

            result
        }
        Err(_) => "fail".to_string(),
    }
}

pub fn get_distro_img(distro: &str) -> String {
    let distro_colours: HashMap<&str, &str> = HashMap::from([
        ("alma", "#dadada"),
        ("alpine", "#2147ea"),
        ("amazon", "#de5412"),
        ("arch", "#12aaff"),
        ("centos", "#ff6600"),
        ("clearlinux", "#56bbff"),
        ("crystal", "#8839ef"),
        ("debian", "#da5555"),
        ("deepin", "#0050ff"),
        ("fedora", "blue"),
        ("gentoo", "#daaada"),
        ("kali", "#000000"),
        ("mageia", "#b612b6"),
        ("mint", "#6fbd20"),
        ("neon", "#27ae60"),
        ("opensuse", "#daff00"),
        ("oracle", "#ff0000"),
        ("redhat", "#ff6662"),
        ("rhel", "#ff6662"),
        ("rocky", "#91ff91"),
        ("slackware", "#6145a7"),
        ("ubuntu", "#FF4400"),
        ("vanilla", "#7f11e0"),
        ("void", "#abff12"),
    ]);

    if distro_colours.contains_key(distro) {
        return format!("<span foreground=\"{}\">⬤</span>", distro_colours[distro]);
    }

    format!("<span foreground=\"{}\">⬤</span>", "#000000")
}

pub fn has_distrobox_installed() -> bool {
    let output = get_command_output(String::from("which"), Some(&["distrobox"]));

    if output.contains("no distrobox in") || output.is_empty() {
        return false;
    }

    true
}

pub fn get_terminal_and_separator_arg() -> (String, String) {
    let mut output = get_command_output(String::from("which"), Some(&["gnome-terminal"]));

    // gnome terminal
    if !output.contains("no gnome-terminal in") && !output.is_empty() {
        return (String::from("gnome-terminal"), String::from("--"));
    }

    // konsole
    output = get_command_output(String::from("which"), Some(&["konsole"]));
    if !output.contains("no konsole in") && !output.is_empty() {
        return (String::from("konsole"), String::from("-e"));
    }

    // tilix
    output = get_command_output(String::from("which"), Some(&["tilix"]));
    if !output.contains("no tilix in") && !output.is_empty() {
        return (String::from("tilix"), String::from("-e"));
    }

    //kitty
    // kitty doesnt have an arg, just `kitty distrobox enter`
    output = get_command_output(String::from("which"), Some(&["kitty"]));
    if !output.contains("no kitty in") && !output.is_empty() {
        return (String::from("kitty"), String::from(""));
    }

    //alacritty
    output = get_command_output(String::from("which"), Some(&["alacritty"]));
    if !output.contains("no alacritty in") && !output.is_empty() {
        return (String::from("alacritty"), String::from("-e"));
    }

    //xterm
    output = get_command_output(String::from("which"), Some(&["xterm"]));
    if !output.contains("no xterm in") && !output.is_empty() {
        return (String::from("xterm"), String::from("-e"));
    }

    (String::from(""), String::from(""))
}

pub fn get_supported_terminals_list() -> String {
    String::from("- Gnome Terminal\n- Konsole\n- Tilix\n- Kitty\n- Alacritty\n- Xterm")
}

pub fn get_container_runtime() -> String {
    let mut runtime = String::from("podman");

    let output = get_command_output(String::from("which"), Some(&["podman"]));
    if output.contains("no podman in") || output.is_empty() {
        runtime = String::from("docker");
    }

    runtime
}

pub fn get_repository_list() -> Vec<String> {
    let runtime = get_container_runtime();

    // podman
    let output = get_command_output(
        runtime,
        Some(&["images", "--format=\"{{.Repository}}:{{.Tag}}\""]),
    );

    return output
        .lines()
        .map(|s| s.trim().replace('"', "").to_string())
        .filter(|s| !s.is_empty())
        .collect();
}

pub fn is_flatpak() -> bool {
    let fp_env = std::env::var("FLATPAK_ID").is_ok();
    if fp_env {
        return true;
    }

    Path::new("/.flatpak-info").exists()
}

pub fn is_nvidia() -> bool {
    let which_lspci = get_command_output(String::from("which"), Some(&["lspci"]));
    if which_lspci.contains("no lspci") || which_lspci.is_empty() {
        // cant detect hardware, assume no
        return false;
    }

    let lspci_output = get_command_output(String::from("lspci"), None);

    let mut has_nvidia = false;

    for line in lspci_output.lines() {
        if line.contains("NVIDIA") {
            has_nvidia = true;
            break;
        }
    }

    has_nvidia
}

pub fn set_up_localisation() {
    textdomain("boxbuddyrs").expect("failed to initialise gettext");
    bind_textdomain_codeset("boxbuddyrs", "UTF-8").expect("failed to bind textdomain for gettext");

    let language_code = env::var("LANG").unwrap_or_else(|_| "en_US".to_string());

    let mut locale_directory = String::from("./po");

    // --TRANSLATORS: Comment out the next 8 lines to test your development locale
    if is_flatpak() {
        locale_directory = String::from("/app/po");
    } else {
        let home_dir = env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let data_home = env::var("XDG_DATA_HOME").unwrap_or_else(|_| format!("{home_dir}/.local/share"));

        locale_directory = format!("{data_home}/locale");
    }

    let locale_directory_path = std::path::PathBuf::from(&locale_directory);
    gettextrs::bindtextdomain("boxbuddyrs", locale_directory_path).expect("a");

    setlocale(LocaleCategory::LcMessages, language_code);
}
