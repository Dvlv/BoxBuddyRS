use gettextrs::*;
use gtk::gio::Settings;
use gtk::prelude::SettingsExt;
use std::collections::HashMap;
use std::env;
use std::path::Path;
use std::process::Command;

use crate::get_all_distroboxes;
use crate::APP_ID;

use crate::config::LOCALEDIR;

pub struct FilesystemAccess {
    pub home: bool,
    pub host: bool,
}

pub struct TerminalOption {
    pub name: String,
    pub executable_name: String,
    pub separator_arg: String,
}

impl FilesystemAccess {
    fn new() -> Self {
        FilesystemAccess {
            home: false,
            host: false,
        }
    }
}

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
        ("fedora", "#3b6db3"),
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

pub fn get_deb_distros() -> Vec<String> {
    vec![
        "debian".to_owned(),
        "deepin".to_owned(),
        "mint".to_owned(),
        "ubuntu".to_owned(),
        "kali".to_owned(),
        "neon".to_owned(),
    ]
}

pub fn get_rpm_distros() -> Vec<String> {
    vec![
        "centos".to_owned(),
        "alma".to_owned(),
        "rocky".to_owned(),
        "fedora".to_owned(),
        "opensuse".to_owned(),
        "oracle".to_owned(),
        "redhat".to_owned(),
        "rhel".to_owned(),
    ]
}

pub fn get_my_deb_boxes() -> Vec<String> {
    let my_boxes = get_all_distroboxes();
    let deb_distros = get_deb_distros();

    let mut my_deb_boxes = Vec::<String>::new();

    for dbox in my_boxes {
        if deb_distros.contains(&dbox.distro) {
            my_deb_boxes.push(dbox.name);
        }
    }

    my_deb_boxes
}

pub fn get_my_rpm_boxes() -> Vec<String> {
    let my_boxes = get_all_distroboxes();
    let rpm_distros = get_rpm_distros();

    let mut my_rpm_boxes = Vec::<String>::new();

    for dbox in my_boxes {
        if rpm_distros.contains(&dbox.distro) {
            my_rpm_boxes.push(dbox.name);
        }
    }

    my_rpm_boxes
}

pub fn has_distrobox_installed() -> bool {
    let output = get_command_output(String::from("which"), Some(&["distrobox"]));

    if output.contains("no distrobox in") || output.is_empty() {
        return false;
    }

    true
}

pub fn get_supported_terminals() -> Vec<TerminalOption> {
    vec![
        TerminalOption {
            name: String::from("GNOME Console"),
            executable_name: String::from("kgx"),
            separator_arg: String::from("--"),
        },
        TerminalOption {
            name: String::from("GNOME Terminal"),
            executable_name: String::from("gnome-terminal"),
            separator_arg: String::from("--"),
        },
        TerminalOption {
            name: String::from("Konsole"),
            executable_name: String::from("konsole"),
            separator_arg: String::from("-e"),
        },
        TerminalOption {
            name: String::from("Tilix"),
            executable_name: String::from("tilix"),
            separator_arg: String::from("-e"),
        },
        TerminalOption {
            name: String::from("Kitty"),
            executable_name: String::from("kitty"),
            separator_arg: String::from(""),
        },
        TerminalOption {
            name: String::from("Alacritty"),
            executable_name: String::from("alacritty"),
            separator_arg: String::from("-e"),
        },
        TerminalOption {
            name: String::from("Xterm"),
            executable_name: String::from("xterm"),
            separator_arg: String::from("-e"),
        },
    ]
}

pub fn get_terminal_and_separator_arg() -> (String, String) {
    let settings = Settings::new(APP_ID);
    let chosen_term = settings.string("default-terminal");

    // first iter through supported terms and find the exec name of their default
    let supported_terminals = get_supported_terminals();
    let mut chosen_term_obj = &supported_terminals[0];
    for term in &supported_terminals {
        if term.name == chosen_term {
            chosen_term_obj = term;
            break;
        }
    }

    let mut output = get_command_output(
        String::from("which"),
        Some(&[&chosen_term_obj.executable_name]),
    );
    let mut potential_error_msg = format!("no {} in", chosen_term_obj.executable_name);

    // if their chosen term is available, return its details
    if !output.contains(&potential_error_msg) && !output.is_empty() {
        return (
            chosen_term_obj.executable_name.clone(),
            chosen_term_obj.separator_arg.clone(),
        );
    }

    // if chosen term is NOT available, iter through list as before
    for term in &supported_terminals {
        output = get_command_output(String::from("which"), Some(&[&term.executable_name]));
        potential_error_msg = format!("no {} in", term.executable_name);

        if !output.contains(&potential_error_msg) && !output.is_empty() {
            return (term.executable_name.clone(), term.separator_arg.clone());
        }
    }

    (String::from(""), String::from(""))
}

pub fn get_supported_terminals_list() -> String {
    let terms = get_supported_terminals();

    terms
        .iter()
        .map(|t| format!("- {}", t.name))
        .collect::<Vec<String>>()
        .join("\n")
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

#[allow(unused_assignments)]
pub fn set_up_localisation() {
    textdomain("boxbuddyrs").expect("failed to initialise gettext");
    bind_textdomain_codeset("boxbuddyrs", "UTF-8").expect("failed to bind textdomain for gettext");

    gettextrs::setlocale(LocaleCategory::LcAll, "");
    gettextrs::bindtextdomain("boxbuddyrs", LOCALEDIR).expect("Unable to bind the text domain");
    gettextrs::textdomain("boxbuddyrs").expect("Unable to switch to the text domain");
}

pub fn get_host_desktop_files() -> Vec<String> {
    let mut host_apps: Vec<String> = Vec::<String>::new();

    if is_flatpak() {
        // we can't use fs in the flatpak sandbox, so parse `ls`.
        let mut data_home =
            get_command_output(String::from("bash"), Some(&["-c", "echo $XDG_DATA_HOME"]));
        if data_home.trim().is_empty() {
            let mut home_dir =
                get_command_output(String::from("bash"), Some(&["-c", "echo $HOME"]));
            home_dir = home_dir.trim().to_string();
            data_home = format!("{home_dir}/.local/share");
        }

        let applications_dir = format!("{data_home}/applications");

        let ls_lines = get_command_output(String::from("ls"), Some(&[applications_dir.as_str()]));

        let desktop_files = ls_lines.split('\n');
        for df in desktop_files {
            if !df.is_empty() {
                host_apps.push(df.to_string());
            }
        }
    } else {
        let home_dir = env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let data_home =
            env::var("XDG_DATA_HOME").unwrap_or_else(|_| format!("{home_dir}/.local/share"));

        let applications_dir = format!("{data_home}/applications");
        let applications_dir_path = std::path::Path::new(&applications_dir);

        if applications_dir_path.exists() {
            let my_apps = std::fs::read_dir(applications_dir_path);
            if let Ok(apps) = my_apps {
                for host_app in apps.flatten() {
                    if let Ok(fname) = host_app.file_name().into_string() {
                        host_apps.push(fname);
                    }
                }
            }
        }
    }

    host_apps
}

pub fn get_flatpak_filesystem_permissions() -> FilesystemAccess {
    let mut access = FilesystemAccess::new();
    // this will check for BoxBuddy installed as a system flatpak
    let sys_output = get_command_output(
        String::from("flatpak"),
        Some(&["override", "--show", "io.github.dvlv.boxbuddyrs"]),
    );
    for line in sys_output.split('\n') {
        if line.starts_with("filesystems=") {
            let fs_overrides = line.replace("filesystems=", "");
            for ovr in fs_overrides.split(';') {
                match ovr {
                    "host" => {
                        access.host = true;
                    }
                    "home" => {
                        access.home = true;
                    }
                    _ => {}
                }
            }
        }
    }

    // check for BoxBuddy as a user flatpak
    let user_output = get_command_output(
        String::from("flatpak"),
        Some(&["override", "--user", "--show", "io.github.dvlv.boxbuddyrs"]),
    );
    for line in user_output.split('\n') {
        if line.starts_with("filesystems=") {
            let fs_overrides = line.replace("filesystems=", "");
            for ovr in fs_overrides.split(';') {
                match ovr {
                    "host" => {
                        access.host = true;
                    }
                    "home" => {
                        access.home = true;
                    }
                    _ => {}
                }
            }
        }
    }

    access
}

pub fn has_host_access() -> bool {
    if is_flatpak() {
        let access = get_flatpak_filesystem_permissions();
        return access.host;
    }

    true
}

pub fn has_home_or_host_access() -> bool {
    if is_flatpak() {
        let access = get_flatpak_filesystem_permissions();
        return access.host || access.home;
    }

    true
}

pub fn get_download_dir_path() -> String {
    env::var("XDG_DOWNLOAD_DIR").unwrap_or_else(|_| {
        let home_dir = env::var("HOME");
        if home_dir.is_err() {
            return String::from("");
        }

        let hme = home_dir.unwrap();
        format!("{hme}/Downloads")
    })
}
