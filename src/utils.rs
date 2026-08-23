use adw::StyleManager;
use gettextrs::{bind_textdomain_codeset, setlocale, textdomain, LocaleCategory};
use gtk::gio::Settings;
use gtk::prelude::{SettingsExt, SettingsExtManual};
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
    pub flatpak_id: Option<String>,
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

/// Best-first alternatives for every icon BoxBuddy asks a theme for. Meant to be
/// passed to `get_available_icon_name`, which picks the first one the user's
/// theme can actually draw.
///
/// The lists cover more than the names known to be missing today, because a
/// theme is free to omit any of them and the cost of an alternative is nothing.
/// The ones we know go missing: `software-update-available-symbolic`,
/// `system-software-install-symbolic`, `application-x-executable-symbolic`,
/// `dialog-warning-symbolic` and `dialog-information-symbolic` are absent from
/// breeze, and `media-playback-stop` is absent from Adwaita.
pub const UPGRADE_ICON_NAMES: &[&str] = &[
    "software-update-available-symbolic",
    "system-software-update-symbolic",
    "system-software-update",
];
pub const WARNING_ICON_NAMES: &[&str] = &["dialog-warning-symbolic", "dialog-warning"];
pub const INFO_ICON_NAMES: &[&str] = &["dialog-information-symbolic", "dialog-information"];
pub const APPLICATIONS_ICON_NAMES: &[&str] = &[
    "application-x-executable-symbolic",
    "application-x-executable",
    "system-run-symbolic",
];
pub const INSTALL_PACKAGE_ICON_NAMES: &[&str] = &[
    "system-software-install-symbolic",
    "system-software-install",
    "install-symbolic",
];
pub const ADD_ICON_NAMES: &[&str] = &["list-add-symbolic", "list-add"];
pub const REMOVE_ICON_NAMES: &[&str] = &["list-remove-symbolic", "list-remove"];
pub const MENU_ICON_NAMES: &[&str] = &["open-menu-symbolic", "open-menu", "view-more-symbolic"];
pub const STOP_ICON_NAMES: &[&str] = &["media-playback-stop-symbolic", "media-playback-stop"];
pub const TERMINAL_ICON_NAMES: &[&str] = &["utilities-terminal-symbolic", "utilities-terminal"];
pub const TRASH_ICON_NAMES: &[&str] =
    &["user-trash-symbolic", "user-trash", "edit-delete-symbolic"];
pub const COPY_ICON_NAMES: &[&str] = &["edit-copy-symbolic", "edit-copy"];
pub const OPEN_FILE_ICON_NAMES: &[&str] = &[
    "document-open-symbolic",
    "document-open",
    "folder-open-symbolic",
];
/// Stands in for BoxBuddy's own Assemble icon if that file cannot be found.
pub const ASSEMBLE_FALLBACK_ICON_NAMES: &[&str] = &[
    "applications-engineering-symbolic",
    "system-run-symbolic",
    "system-run",
];

/// Returns whichever of `icon_names` the user's icon theme is actually able to
/// draw, so a button does not end up showing a broken-image placeholder.
///
/// GTK does not quietly fall back to Adwaita for a name the current theme has
/// never heard of, and several themes are missing names BoxBuddy asks for -
/// breeze, the KDE default, has no `software-update-available-symbolic` - so we
/// list the alternatives we know about and pick one that exists. Returns the
/// first name if none of them are found, leaving the behaviour as it was.
pub fn get_available_icon_name(icon_names: &[&str]) -> String {
    if let Some(display) = gtk::gdk::Display::default() {
        let theme = gtk::IconTheme::for_display(&display);

        for name in icon_names {
            if theme.has_icon(name) {
                return (*name).to_string();
            }
        }
    }

    icon_names[0].to_string()
}

/// Like `get_available_icon_name`, but for an icon name BoxBuddy has no say over:
/// the `Icon=` line of a desktop file found inside a box.
///
/// Such an icon is installed in the box rather than on the host, so the host's
/// theme has usually never heard of it - and the desktop file is even allowed to
/// name a file path rather than a theme icon - which would leave a broken-image
/// placeholder next to the application. Fall back to a generic application icon.
pub fn get_available_app_icon_name(desktop_file_icon: &str) -> String {
    let mut icon_names = vec![desktop_file_icon];
    icon_names.extend_from_slice(APPLICATIONS_ICON_NAMES);

    get_available_icon_name(&icon_names)
}

/// Distro brand colours, shared by the coloured dot on the notebook tab and
/// the coloured bar in the box header, so the two can never disagree.
const DISTRO_COLOURS: [(&str, &str); 24] = [
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
];

/// Looks up the brand colour for a distribution, returning a CSS colour
/// string (`#rrggbb`). Falls back to black for unknown distros.
pub fn get_distro_color(distro: &str) -> &'static str {
    DISTRO_COLOURS
        .iter()
        .find(|(name, _)| *name == distro)
        .map_or("#000000", |(_, colour)| colour)
}

/// CSS for the coloured bar in each box header: a base class plus one
/// `.distro-color-bar-<name>` override per known distro, generated from the
/// same table as the tab dot. Meant to be loaded into the display once, not
/// per box.
pub fn get_distro_color_css() -> String {
    let mut css = String::from(
        ".distro-color-bar { background-color: #000000; border-radius: 2px; min-height: 32px; min-width: 4px; }\n",
    );
    for (name, colour) in DISTRO_COLOURS {
        css.push_str(&format!(
            ".distro-color-bar-{name} {{ background-color: {colour}; }}\n"
        ));
    }
    css
}

/// Gets the unicode dot character coloured with a colour similar to the distro's branding
pub fn get_distro_img(distro: &str) -> String {
    format!("<span foreground=\"{}\">⬤</span>", get_distro_color(distro))
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

/// The package manager of a given container image. Detected by matching the
/// image name (e.g. `docker.io/library/archlinux:latest`) against the same
/// set of regexes Kontainer uses - see
/// `packageinstallcommand.cpp:16-50` upstream. We deliberately avoid pulling
/// in a regex crate: `str::contains` with a few alternatives per line is
/// enough for the shapes distrobox upstream ships today, and it keeps the
/// dep graph clean.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PkgManager {
    Apt,
    Dnf,
    Zypper,
    Pacman,
    Apk,
    Xbps,
    Emerge,
    Installpkg,
}

/// Detects the package manager used inside a container by inspecting its
/// image name. Returns `None` for images that do not match any of the
/// patterns - the caller is then responsible for falling back to a safe
/// default or refusing the operation outright.
pub fn detect_pkg_manager(image: &str) -> Option<PkgManager> {
    let lower = image.to_lowercase();
    // Order matters only in the sense that more specific matches come
    // first. All distros within one family share a manager, so the order
    // between families does not.
    if lower.contains("fedora")
        || lower.contains("bluefin")
        || lower.contains("ublue-os/fedora")
        || lower.contains("fedoraproject.org/fedora")
    {
        Some(PkgManager::Dnf)
    } else if lower.contains("ubuntu")
        || lower.contains("toolbx/ubuntu")
        || lower.contains("ubuntu-toolbox")
        || lower.contains("debian")
        || lower.contains("neurodebian")
        || lower.contains("mint")
        || lower.contains("kali")
        || lower.contains("neon")
    {
        Some(PkgManager::Apt)
    } else if lower.contains("opensuse") || lower.contains("tumbleweed") || lower.contains("leap") {
        Some(PkgManager::Zypper)
    } else if lower.contains("arch")
        || lower.contains("blackarch")
        || lower.contains("ublue-os/arch")
        || lower.contains("bazzite-arch")
        || lower.contains("arch-toolbox")
    {
        Some(PkgManager::Pacman)
    } else if lower.contains("centos")
        || lower.contains("rhel")
        || lower.contains("rocky")
        || lower.contains("alma")
        || lower.contains("ubi")
        || lower.contains("amazonlinux")
        || lower.contains("oracle")
    {
        Some(PkgManager::Dnf)
    } else if lower.contains("alpine") {
        Some(PkgManager::Apk)
    } else if lower.contains("void") {
        Some(PkgManager::Xbps)
    } else if lower.contains("gentoo") {
        Some(PkgManager::Emerge)
    } else if lower.contains("slack") {
        Some(PkgManager::Installpkg)
    } else if lower.contains("wolfi") || lower.contains("chainguard") {
        Some(PkgManager::Apk)
    } else {
        None
    }
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
            flatpak_id: None,
        },
        TerminalOption {
            name: String::from("GNOME Terminal"),
            executable_name: String::from("gnome-terminal"),
            separator_arg: String::from("--"),
            flatpak_id: None,
        },
        TerminalOption {
            name: String::from("Konsole"),
            executable_name: String::from("konsole"),
            separator_arg: String::from("-e"),
            flatpak_id: Some(String::from("org.kde.konsole")),
        },
        TerminalOption {
            name: String::from("Xfce Terminal"),
            executable_name: String::from("xfce4-terminal"),
            separator_arg: String::from("-x"),
            flatpak_id: None,
        },
        TerminalOption {
            name: String::from("Tilix"),
            executable_name: String::from("tilix"),
            separator_arg: String::from("-e"),
            flatpak_id: None,
        },
        TerminalOption {
            name: String::from("Kitty"),
            executable_name: String::from("kitty"),
            separator_arg: String::from("--"),
            flatpak_id: None,
        },
        TerminalOption {
            name: String::from("Alacritty"),
            executable_name: String::from("alacritty"),
            separator_arg: String::from("-e"),
            flatpak_id: None,
        },
        TerminalOption {
            name: String::from("WezTerm"),
            executable_name: String::from("wezterm"),
            separator_arg: String::from("-e"),
            flatpak_id: Some(String::from("org.wezfurlong.wezterm")),
        },
        TerminalOption {
            name: String::from("Ghostty"),
            executable_name: String::from("ghostty"),
            separator_arg: String::from("-e"),
            flatpak_id: None,
        },
        TerminalOption {
            name: String::from("elementary Terminal"),
            executable_name: String::from("io.elementary.terminal"),
            separator_arg: String::from("--"),
            flatpak_id: None,
        },
        TerminalOption {
            name: String::from("Ptyxis"),
            executable_name: String::from("ptyxis"),
            separator_arg: String::from("--"),
            flatpak_id: Some(String::from("app.devsuite.Ptyxis")),
        },
        TerminalOption {
            name: String::from("Foot"),
            executable_name: String::from("footclient"),
            separator_arg: String::from("-e"),
            flatpak_id: None,
        },
        TerminalOption {
            name: String::from("Terminator"),
            executable_name: String::from("terminator"),
            separator_arg: String::from("-x"),
            flatpak_id: None,
        },
        TerminalOption {
            name: String::from("Deepin Terminal"),
            executable_name: String::from("deepin-terminal"),
            separator_arg: String::from("-e"),
            flatpak_id: None,
        },
        TerminalOption {
            name: String::from("Xterm"),
            executable_name: String::from("xterm"),
            separator_arg: String::from("-e"),
            flatpak_id: None,
        },
        TerminalOption {
            name: String::from("COSMIC Terminal"),
            executable_name: String::from("cosmic-term"),
            separator_arg: String::from("-e"),
            flatpak_id: None,
        },
    ]
}

/// Returns the executable command and separator arg for the terminal which
/// `BoxBuddy` will spawn. First tries to find the Preferred Terminal, if set,
/// then loops through all options in order if it can't.
/// Returns a tuple of the terminal exec, the terminal separator arg, and a boolean
/// of whether this terminal is a flatpak.
/// If the terminal IS a flatpak, the first tuple element will be the flatpak
/// ID, but if it's NOT a flatpak it will be the executable name
/// Returns two empty strings if no supported terminal can be detected
pub fn get_terminal_and_separator_arg() -> (String, String, bool) {
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
            false,
        );
    }

    // if their term is NOT available, check if it is a flatpak
    if chosen_term_obj.flatpak_id.is_some() {
        let user_flatpaks = get_users_supported_terminal_flatpaks();
        if user_flatpaks.contains(&chosen_term_obj.flatpak_id.as_ref().unwrap()) {
            return (
                chosen_term_obj.flatpak_id.as_ref().unwrap().clone(),
                chosen_term_obj.separator_arg.clone(),
                true,
            );
        }
    }

    // if chosen term is NOT available at all, iter through list as before
    for term in &supported_terminals {
        output = get_command_output("which", Some(&[&term.executable_name]));
        potential_error_msg = format!("no {} in", term.executable_name);

        if !output.contains(&potential_error_msg) && !output.is_empty() {
            return (
                term.executable_name.clone(),
                term.separator_arg.clone(),
                false,
            );
        }
    }

    (String::new(), String::new(), false)
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

/// Returns a Vec of flatpak IDs of any supported terminals which are installed
pub fn get_users_supported_terminal_flatpaks() -> Vec<String> {
    // first check if they have flatpak at all
    let mut has_fp_out = get_command_output("which", Some(&["flatpak"]));
    if has_fp_out.contains("no flatpak in") || has_fp_out.is_empty() {
        return Vec::new();
    }

    let output = get_command_output("flatpak", Some(&["list", "--columns=app"]));

    let term_flatpak_ids: Vec<String> = get_supported_terminals()
        .iter()
        .map(|t| &t.flatpak_id)
        .filter(|f| f.is_some())
        .map(|t| t.as_ref().unwrap().clone())
        .collect();

    let mut user_flatpak_terms = Vec::<String>::new();

    for line in output.lines() {
        let line_string = String::from(line.trim());
        if term_flatpak_ids.contains(&line_string) {
            user_flatpak_terms.push(line_string);
        }
    }

    user_flatpak_terms
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
    /*if is_flatpak() {
        locale_directory = String::from("/app/po");
    } else {
        let home_dir = env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let data_home =
            env::var("XDG_DATA_HOME").unwrap_or_else(|_| format!("{home_dir}/.local/share"));

        locale_directory = format!("{data_home}/locale");
    }*/

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

/// The custom menu-label alias the user set for a box, or `None` for the
/// distrobox default. Stored per box in GSettings, keyed by box name.
pub fn get_exported_app_label(box_name: &str) -> Option<String> {
    let settings = Settings::new(APP_ID);
    let labels: HashMap<String, String> = settings.get("exported-app-labels");
    labels
        .get(box_name)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Sets (or, for an empty value, clears) the custom menu-label alias for a box.
/// Clearing it means exports fall back to distrobox's own "(on <box>)" label.
pub fn set_exported_app_label(box_name: &str, label: &str) {
    let settings = Settings::new(APP_ID);
    let mut labels: HashMap<String, String> = settings.get("exported-app-labels");
    let trimmed = label.trim();
    if trimmed.is_empty() {
        labels.remove(box_name);
    } else {
        labels.insert(box_name.to_string(), trimmed.to_string());
    }
    let _ = settings.set("exported-app-labels", &labels);
}

#[cfg(test)]
mod tests {
    use super::{detect_pkg_manager, PkgManager};

    /// Every image URL here is one distrobox actually offers in
    /// `distrobox create --compatibility`, plus the docker.io shorthands
    /// people type by hand.
    #[test]
    fn detects_manager_for_real_image_urls() {
        let cases = [
            ("docker.io/library/ubuntu:latest", PkgManager::Apt),
            ("quay.io/toolbx/ubuntu-toolbox:24.04", PkgManager::Apt),
            ("docker.io/library/debian:12", PkgManager::Apt),
            ("docker.io/kalilinux/kali-rolling", PkgManager::Apt),
            ("linuxmintd/mint21.3-amd64", PkgManager::Apt),
            ("quay.io/fedora/fedora:43", PkgManager::Dnf),
            (
                "registry.fedoraproject.org/fedora-toolbox:latest",
                PkgManager::Dnf,
            ),
            ("ghcr.io/ublue-os/bluefin-cli", PkgManager::Dnf),
            ("quay.io/centos/centos:stream9", PkgManager::Dnf),
            ("registry.access.redhat.com/ubi9/ubi", PkgManager::Dnf),
            ("quay.io/rockylinux/rockylinux:9", PkgManager::Dnf),
            ("docker.io/library/almalinux:9", PkgManager::Dnf),
            (
                "public.ecr.aws/amazonlinux/amazonlinux:2023",
                PkgManager::Dnf,
            ),
            (
                "container-registry.oracle.com/os/oraclelinux:9",
                PkgManager::Dnf,
            ),
            (
                "registry.opensuse.org/opensuse/tumbleweed:latest",
                PkgManager::Zypper,
            ),
            (
                "registry.opensuse.org/opensuse/leap:15.6",
                PkgManager::Zypper,
            ),
            ("docker.io/library/archlinux:latest", PkgManager::Pacman),
            (
                "docker.io/blackarchlinux/blackarch:latest",
                PkgManager::Pacman,
            ),
            ("docker.io/library/alpine:3.20", PkgManager::Apk),
            ("cgr.dev/chainguard/wolfi-base", PkgManager::Apk),
            ("ghcr.io/void-linux/void-glibc:latest", PkgManager::Xbps),
            ("docker.io/gentoo/stage3:latest", PkgManager::Emerge),
            ("docker.io/vbatts/slackware:current", PkgManager::Installpkg),
        ];

        for (image, expected) in cases {
            assert_eq!(
                detect_pkg_manager(image),
                Some(expected),
                "wrong manager for {image}"
            );
        }
    }

    #[test]
    fn unknown_images_detect_nothing() {
        assert_eq!(detect_pkg_manager("docker.io/library/hello-world"), None);
        assert_eq!(detect_pkg_manager(""), None);
    }

    /// The match is case-insensitive, since registries are but tags are not
    /// always typed that way.
    #[test]
    fn detection_ignores_case() {
        assert_eq!(
            detect_pkg_manager("docker.io/library/Ubuntu:LATEST"),
            Some(PkgManager::Apt)
        );
    }
}
