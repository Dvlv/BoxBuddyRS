use adw::StyleManager;
use gettextrs::{bind_textdomain_codeset, setlocale, textdomain, LocaleCategory};
use gtk::gio::Settings;
use gtk::prelude::SettingsExt;
use std::collections::HashMap;
use std::env;
use std::path::Path;
use std::process::Command;

use crate::get_all_distroboxes;
use crate::APP_ID;

/// Used to represent any Filesystem overrides granted to the Flatpak
/// instance of `BoxBuddy`
pub struct FilesystemAccess {
    /// Whether or not the user has granted `home` access
    pub home: bool,
    /// Whether or not the user has granted `host` access
    pub host: bool,
}

/// Used to represent terminals `BoxBuddy` can spawn
pub struct TerminalOption {
    /// Public-facing name of the terminal
    pub name: String,
    /// Command to execute to spawn the terminal
    pub executable_name: String,
    /// Argument provided to separate the terminal spawning from the command it should run
    pub separator_arg: String,
}

/// Used to represent the resources used by a container
pub struct CpuMemUsage {
    /// CPU usage
    pub cpu: String,
    /// Mem usage
    pub mem: String,
    /// Mem percentage usage
    pub mem_percent: String,
}

impl FilesystemAccess {
    fn new() -> Self {
        FilesystemAccess {
            home: false,
            host: false,
        }
    }
}

/// Runs shell command. Uses flatpak-spawn if `BoxBuddy` is running as a Flatpak
pub fn run_command(
    cmd_to_run: &str,
    args_for_cmd: Option<&[&str]>,
) -> Result<std::process::Output, std::io::Error> {
    let mut cmd = Command::new(cmd_to_run);

    if is_flatpak() {
        cmd = Command::new("flatpak-spawn");
        cmd.arg("--host");
        cmd.arg(cmd_to_run);
    }

    if let Some(a) = args_for_cmd {
        cmd.args(a);
    }

    cmd.output()
}

/// Runs shell command and returns the output as a string
pub fn get_command_output(cmd_to_run: &str, args_for_cmd: Option<&[&str]>) -> String {
    let output = run_command(cmd_to_run, args_for_cmd);

    match output {
        Ok(o) => {
            let mut result = String::new();
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

/// Runs shell command and returns the output as a string, but does NOT
/// return stderr.
pub fn get_command_output_no_err(cmd_to_run: &str, args_for_cmd: Option<&[&str]>) -> String {
    let output = run_command(cmd_to_run, args_for_cmd);

    match output {
        Ok(o) => {
            let mut result = String::new();
            if !o.stdout.is_empty() {
                result = result
                    + String::from_utf8_lossy(&o.stdout).into_owned().as_ref()
                    + &String::from("\n");
            }

            result
        }
        Err(_) => "fail".to_string(),
    }
}

/// Checks if the extension of a file (passed as a string) corresponds to a given string.
/// Case insensitive.
pub fn has_file_extension(path: &str, extension: &str) -> bool {
    Path::new(path)
        .extension()
        .map_or(false, |ext| ext.eq_ignore_ascii_case(extension))
}

/// Gets the unicode dot character coloured with a colour similar to the distro's branding
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

/// Returns a vector of distros which can install .deb packages
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

/// Returns a vector of distros which can install .rpm packages
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

/// Returns a vector of the user's distroboxes which can install .deb packages
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

/// Returns a vector of the user's distroboxes which can install .rpm packages
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

/// Whether or not the `distrobox` command can be successfully run
pub fn has_distrobox_installed() -> bool {
    let output = get_command_output("which", Some(&["distrobox"]));

    if output.contains("no distrobox in") || output.is_empty() {
        return false;
    }

    true
}

/// Whether or not the `podman` or `docker` command can be successfully run
pub fn has_podman_or_docker_installed() -> bool {
    let output = get_command_output("which", Some(&["podman"]));

    if output.contains("no podman in") || output.is_empty() {
        let docker_output = get_command_output("which", Some(&["docker"]));

        if docker_output.contains("no docker in") || docker_output.is_empty() {
            return false;
        }
    }

    true
}

/// Returns a Vec of `TerminalOption`s representing all terminals supported by `BoxBuddy`
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
            name: String::from("Xfce Terminal"),
            executable_name: String::from("xfce4-terminal"),
            separator_arg: String::from("-x"),
        },
        TerminalOption {
            name: String::from("Tilix"),
            executable_name: String::from("tilix"),
            separator_arg: String::from("-e"),
        },
        TerminalOption {
            name: String::from("Kitty"),
            executable_name: String::from("kitty"),
            separator_arg: String::from("--"),
        },
        TerminalOption {
            name: String::from("Alacritty"),
            executable_name: String::from("alacritty"),
            separator_arg: String::from("-e"),
        },
        TerminalOption {
            name: String::from("WezTerm"),
            executable_name: String::from("wezterm"),
            separator_arg: String::from("-e"),
        },
        TerminalOption {
            name: String::from("Ptyxis"),
            executable_name: String::from("ptyxis"),
            separator_arg: String::from("--"),
        },
        TerminalOption {
            name: String::from("Foot"),
            executable_name: String::from("footclient"),
            separator_arg: String::from("-e"),
        },
        TerminalOption {
            name: String::from("Xterm"),
            executable_name: String::from("xterm"),
            separator_arg: String::from("-e"),
        },
        TerminalOption {
            name: String::from("COSMIC Terminal"),
            executable_name: String::from("cosmic-term"),
            separator_arg: String::from("-e"),
        },
    ]
}

/// Returns the executable command and separator arg for the terminal which
/// `BoxBuddy` will spawn. First tries to find the Preferred Terminal, if set,
/// then loops through all options in order if it can't.
/// Returns two empty strings if no supported terminal can be detected
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

    let mut output = get_command_output("which", Some(&[&chosen_term_obj.executable_name]));
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
        output = get_command_output("which", Some(&[&term.executable_name]));
        potential_error_msg = format!("no {} in", term.executable_name);

        if !output.contains(&potential_error_msg) && !output.is_empty() {
            return (term.executable_name.clone(), term.separator_arg.clone());
        }
    }

    (String::new(), String::new())
}

/// Returns a single string of a bullet-pointed list of supported terminals
/// for display to the user if no supported terminal is found.
pub fn get_supported_terminals_list() -> String {
    let terms = get_supported_terminals();

    terms
        .iter()
        .map(|t| format!("- {}", t.name))
        .collect::<Vec<String>>()
        .join("\n")
}

/// Returns "podman" or "docker", based on which is installed, for use by
/// `get_repository_list` below
pub fn get_container_runtime() -> String {
    let mut runtime = String::from("podman");

    let output = get_command_output("which", Some(&["podman"]));
    if output.contains("no podman in") || output.is_empty() {
        runtime = String::from("docker");
    }

    runtime
}

/// Gets CPU and Memory used for each box.
/// In here instead of Distrobox Handler because we have
/// to shell out to the actual runtime.
pub fn get_cpu_and_mem_usage(box_name: &str) -> CpuMemUsage {
    let runtime = get_container_runtime();
    let stats_output = get_command_output_no_err(
        &runtime,
        Some(&[
            "stats",
            box_name,
            "--no-stream",
            "--format",
            "{{.CPUPerc}};{{.MemPerc}};{{.MemUsage}}",
        ]),
    );

    let output_pieces: Vec<&str> = stats_output.split(';').collect();
    if output_pieces.len() != 3 {
        // We failed to get the output for some reason
        return CpuMemUsage {
            cpu: String::new(),
            mem: String::new(),
            mem_percent: String::new(),
        };
    }

    CpuMemUsage {
        cpu: output_pieces[0].trim().to_string(),
        mem: output_pieces[1].trim().to_string(),
        mem_percent: output_pieces[2].trim().to_string(),
    }
}

/// Returns a Vec of "image:version" strings for all container images already
/// downloaded. This is used to show the symbol next to downloaded container
/// images on the Image select when creating a new box
pub fn get_repository_list() -> Vec<String> {
    let runtime = get_container_runtime();

    // podman
    let output = get_command_output(
        &runtime,
        Some(&["images", "--format=\"{{.Repository}}:{{.Tag}}\""]),
    );

    return output
        .lines()
        .map(|s| s.trim().replace('"', "").to_string())
        .filter(|s| !s.is_empty())
        .collect();
}

/// Whether or not `BoxBuddy` is running as a Flatpak
pub fn is_flatpak() -> bool {
    let fp_env = std::env::var("FLATPAK_ID").is_ok();
    if fp_env {
        return true;
    }

    Path::new("/.flatpak-info").exists()
}

/// Whether or not the user appears to have an NVIDIA card, used to pass
/// the --nvidia flag when creating a new box.
pub fn is_nvidia() -> bool {
    let which_lspci = get_command_output("which", Some(&["lspci"]));
    if which_lspci.contains("no lspci") || which_lspci.is_empty() {
        // cant detect hardware, assume no
        return false;
    }

    let lspci_output = get_command_output("lspci", None);

    let mut has_nvidia = false;

    for line in lspci_output.lines() {
        if line.contains("NVIDIA") {
            has_nvidia = true;
            break;
        }
    }

    has_nvidia
}

/// Set up gettext
#[allow(unused_assignments)]
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
        let data_home =
            env::var("XDG_DATA_HOME").unwrap_or_else(|_| format!("{home_dir}/.local/share"));

        locale_directory = format!("{data_home}/locale");
    }

    let locale_directory_path = std::path::PathBuf::from(&locale_directory);
    gettextrs::bindtextdomain("boxbuddyrs", locale_directory_path).expect("a");

    setlocale(LocaleCategory::LcMessages, language_code);
}

/// Gets list of .desktop files on the host system which may have been exported from
/// a box. This is to determine whether to show the "Remove from Menu" button on the
/// View Applications pop-up
pub fn get_host_desktop_files() -> Vec<String> {
    let mut host_apps: Vec<String> = Vec::<String>::new();

    if is_flatpak() {
        // we can't use fs in the flatpak sandbox, so parse `ls`.
        let mut data_home = get_command_output("bash", Some(&["-c", "echo $XDG_DATA_HOME"]));
        if data_home.trim().is_empty() {
            let mut home_dir = get_command_output("bash", Some(&["-c", "echo $HOME"]));
            home_dir = home_dir.trim().to_string();
            data_home = format!("{home_dir}/.local/share");
        }

        let applications_dir = format!("{data_home}/applications");

        let ls_lines = get_command_output("ls", Some(&[applications_dir.as_str()]));

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
        let applications_dir_path = Path::new(&applications_dir);

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

/// Returns a struct which allows us to determine whether the user has added
/// a `home` or `host` Filesystem override to a Flatpak install.
/// This lets us disable features which won't work without these permissions.
pub fn get_flatpak_filesystem_permissions() -> FilesystemAccess {
    let mut access = FilesystemAccess::new();
    // this will check for BoxBuddy installed as a system flatpak
    let sys_output = get_command_output(
        "flatpak",
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
        "flatpak",
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

/// Returns whether or not the user has added a `host` Filesystem override.
pub fn has_host_access() -> bool {
    if is_flatpak() {
        let access = get_flatpak_filesystem_permissions();
        return access.host;
    }

    true
}

/// Returns whether or not the user has added a `host` or `home` Filesystem override.
pub fn has_home_or_host_access() -> bool {
    if is_flatpak() {
        let access = get_flatpak_filesystem_permissions();
        return access.host || access.home;
    }

    true
}

/// Gets the path to icons which are not part of GTK
#[allow(unreachable_code)]
pub fn get_icon_file_path(icon: &str) -> String {
    if is_flatpak() {
        return format!("/app/icons/{icon}");
    }

    // Runs only when developing
    debug_assert!({
        return format!("icons/{icon}");
    });

    let home_dir = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let data_home =
        env::var("XDG_DATA_HOME").unwrap_or_else(|_| format!("{home_dir}/.local/share"));

    format!("{data_home}/icons/boxbuddy/{icon}")
}

/// Get the path to the icon used in the Assemble button. Gets a light
/// or dark icon depending on the user's GTK theme.
pub fn get_assemble_icon() -> String {
    if is_dark_mode() {
        return get_icon_file_path("build-alt-symbolic-light.svg");
    }

    get_icon_file_path("build-alt-symbolic.svg")
}

/// Whether or not the user is using a Dark GTK theme
pub fn is_dark_mode() -> bool {
    StyleManager::default().is_dark()
}

/// Tries to find the path to the user's Download dir.
pub fn get_download_dir_path() -> String {
    env::var("XDG_DOWNLOAD_DIR").unwrap_or_else(|_| {
        let home_dir = env::var("HOME");
        if home_dir.is_err() {
            return String::new();
        }

        let hme = home_dir.unwrap();
        format!("{hme}/Downloads")
    })
}
