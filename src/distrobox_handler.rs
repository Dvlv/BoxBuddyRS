use crate::utils::{
    get_command_output, get_host_desktop_files, get_repository_list,
    get_terminal_and_separator_arg, is_flatpak, is_nvidia, run_command,
};
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;

/// Struct representing a distrobox installed on the user's machine
pub struct DBox {
    /// Name of the box
    pub name: String,
    /// The container image distro
    pub distro: String,
    /// The url for the container image
    pub image_url: String,
    /// The unique ID for this container
    pub container_id: String,
    /// The status of this box
    pub status: String,
    /// Whether or not this box is running (used to show/hide the Stop button)
    pub is_running: bool,
}

/// Struct representing an application installed in a box
#[derive(Debug, Clone)]
pub struct DBoxApp {
    /// User-facing name of the application
    pub name: String,
    /// Executable command to run the application
    pub exec_name: String,
    /// Icon name of the application
    pub icon: String,
    /// Path to the desktop file
    pub desktop_file: String,
    /// Whether or not this app has been exported to the host
    pub is_on_host: bool,
}

/// Used to determine which column contains which data when splitting
/// the output of `distrobox list`, since it changes between versions.
pub struct ColsIndexes {
    /// Column index of the Name heading
    pub name: usize,
    /// Column index of the Image heading
    pub image: usize,
    /// Column index of the ID heading
    pub id: usize,
    /// Column index of the Status heading
    pub status: usize,
}

/// Returns a Vec of all distroboxes belonging to the user
#[allow(clippy::useless_asref)]
pub fn get_all_distroboxes() -> Vec<DBox> {
    let mut my_boxes: Vec<DBox> = vec![];

    let output = get_command_output("distrobox", Some(&["list", "--no-color"]));

    let headings = output
        .split('\n')
        .next()
        .unwrap()
        .split('|')
        .map(str::trim)
        .collect::<Vec<&str>>();
    //println!("headings: {:?}", headings);

    let mut heading_indexes = ColsIndexes {
        name: 1,
        image: 3,
        id: 0,
        status: 2,
    };

    for (idx, heading) in headings.iter().enumerate() {
        match heading.as_ref() {
            "NAME" => heading_indexes.name = idx,
            "IMAGE" => heading_indexes.image = idx,
            "ID" => heading_indexes.id = idx,
            "STATUS" => heading_indexes.status = idx,
            _ => (),
        }
    }

    for (idx, line) in output.split('\n').enumerate() {
        if line.is_empty() || idx == 0 {
            continue;
        }

        let box_line = line.split('|').map(str::trim).collect::<Vec<&str>>();
        if box_line.len() > 3 && is_short_container_id(box_line[heading_indexes.id]) {
            let status = String::from(box_line[heading_indexes.status]);
            let is_running = !status.contains("Exited") && !status.contains("Created");

            my_boxes.push(DBox {
                name: String::from(box_line[heading_indexes.name]),
                distro: try_parse_distro_name_from_url(box_line[heading_indexes.image]),
                image_url: String::from(box_line[heading_indexes.image]),
                container_id: String::from(box_line[heading_indexes.id]),
                status,
                is_running,
            });
        }
    }

    my_boxes
}

/// Whether a column holds the 12-character short container ID which every row of
/// `distrobox list` starts with.
///
/// This is how we tell a real row apart from a fragment of one. `distrobox list`
/// asks the container runtime for the labels and mounts of each container so it
/// can spot the mounts distrobox itself adds, then walks that output a line at a
/// time. A label value containing a newline - the description label of
/// `docker.io/library/ubuntu:latest` is one - therefore spreads a single
/// container over several lines, and distrobox prints any of those fragments
/// which happens to mention distrobox as though it were a container of its own.
/// Such a fragment still holds enough pipes to parse, so without this check it
/// becomes a box whose name is a wall of label text.
fn is_short_container_id(column: &str) -> bool {
    column.len() == 12 && column.chars().all(|c| c.is_ascii_hexdigit())
}

/// Tries to figure out the distro name of a repository URL. Returns "zunknown" if it can't
/// It's "zunknown" so that it's alphabetically last.
pub fn try_parse_distro_name_from_url(url: &str) -> String {
    let distros = [
        "alma",
        "alpine",
        "amazon",
        "bazzite", // needs to be before arch because the image is bazzite-arch
        "arch",
        "centos",
        "clearlinux",
        "crystal",
        "debian",
        "deepin",
        "fedora",
        "gentoo",
        "kali",
        "mageia",
        "mint",
        "neon",
        "opensuse",
        "oracle",
        "redhat",
        "rhel",
        "rocky",
        "slackware",
        "steamos",
        "ubuntu",
        "ublue",
        "vanilla",
        "void",
    ];

    let mut distro_name = "zunknown";

    let last_part_of_url = url.split('/').last().unwrap_or("zunknown");

    for d in distros {
        if last_part_of_url.contains(d) {
            distro_name = d;
            break;
        }
    }

    if distro_name != "zunknown" {
        return distro_name.to_string();
    }

    for d in distros {
        if url.contains(d) {
            distro_name = d;
            break;
        }
    }

    distro_name.to_string()
}

/// Spawns a terminal running inside the provided box.
pub fn open_terminal_in_box(box_name: String) {
    let (term, sep, term_is_flatpak) = get_terminal_and_separator_arg();

    if is_flatpak() {
        if term_is_flatpak {
            Command::new("flatpak-spawn")
                .arg("--host")
                .arg("flatpak")
                .arg("run")
                .arg(term)
                .arg(sep)
                .arg("distrobox")
                .arg("enter")
                .arg(box_name)
                .spawn()
                .unwrap();
        } else {
            Command::new("flatpak-spawn")
                .arg("--host")
                .arg(term)
                .arg(sep)
                .arg("distrobox")
                .arg("enter")
                .arg(box_name)
                .spawn()
                .unwrap();
        }
    } else {
        if term_is_flatpak {
            Command::new("flatpak")
                .arg("run")
                .arg(term)
                .arg(sep)
                .arg("distrobox")
                .arg("enter")
                .arg(box_name)
                .spawn()
                .unwrap();
        } else {
            Command::new(term)
                .arg(sep)
                .arg("distrobox")
                .arg("enter")
                .arg(box_name)
                .spawn()
                .unwrap();
        }
    }
}

/// Exports the desktop file from a box.
pub fn export_app_from_box(app_name: &str, box_name: &str) -> String {
    get_command_output(
        "distrobox",
        Some(&[
            "enter",
            box_name,
            "--",
            "distrobox-export",
            "--app",
            app_name,
        ]),
    )
}

/// Unexports a desktop file from the host.
pub fn remove_app_from_host(app_name: &str, box_name: &str) -> String {
    get_command_output(
        "distrobox",
        Some(&[
            "enter",
            box_name,
            "--",
            "distrobox-export",
            "--app",
            app_name,
            "--delete",
        ]),
    )
}

/// Runs a command inside a box using `distrobox enter --`. Does NOT spawn terminal.
pub fn run_command_in_box(command: &str, box_name: &str) {
    if is_flatpak() {
        Command::new(String::from("flatpak-spawn"))
            .args(["--host", "distrobox", "enter", box_name, "--", command])
            .spawn()
            .unwrap();
    } else {
        Command::new(String::from("distrobox"))
            .args(["enter", box_name, "--", command])
            .spawn()
            .unwrap();
    }
}

/// Performs `distrobox upgrade` inside a box.
/// Spawns a terminal, and runs `distrobox enter` afterwards just so the terminal
/// stays open.
pub fn upgrade_box(box_name: &str) {
    let (term, sep, term_is_flatpak) = get_terminal_and_separator_arg();
    let command = format!("distrobox upgrade {box_name}; distrobox enter {box_name}");

    if is_flatpak() {
        if term_is_flatpak {
            Command::new("flatpak-spawn")
                .arg("--host")
                .arg("flatpak")
                .arg("run")
                .arg(term)
                .arg(sep)
                .arg("bash")
                .arg("-c")
                .arg(&command)
                .spawn()
                .unwrap();
        } else {
            Command::new("flatpak-spawn")
                .arg("--host")
                .arg(term)
                .arg(sep)
                .arg("bash")
                .arg("-c")
                .arg(&command)
                .spawn()
                .unwrap();
        }
    } else {
        if term_is_flatpak {
            Command::new("flatpak")
                .arg("run")
                .arg(term)
                .arg(sep)
                .arg("bash")
                .arg("-c")
                .arg(&command)
                .spawn()
                .unwrap();
        } else {
            Command::new(term)
                .arg(sep)
                .arg("bash")
                .arg("-c")
                .arg(&command)
                .spawn()
                .unwrap();
        }
    }
}

pub fn delete_box(box_name: &str) -> String {
    get_command_output("distrobox", Some(&["rm", box_name, "--force"]))
}

/// Creates a new distrobox, spawns a terminal with `distrobox enter` afterwards
/// to initialise it.
///
/// An empty `home_path` or `hostname` means the flag is left off entirely, so
/// distrobox applies its own default.
pub fn create_box(
    box_name: &str,
    image: &str,
    home_path: &str,
    hostname: &str,
    use_init: bool,
    volumes: &[String],
) -> String {
    let mut args = vec!["create", "-n", box_name, "-i", image, "-Y"];
    if is_nvidia() {
        args.push("--nvidia");
    }

    if use_init {
        args.push("--init");
        args.push("--additional-packages");
        args.push("systemd");
    }

    if !home_path.is_empty() {
        args.push("--home");
        args.push(home_path);
    }

    if !hostname.is_empty() {
        args.push("--hostname");
        args.push(hostname);
    }

    if !volumes.is_empty() {
        for vol in volumes {
            args.push("--volume");
            args.push(vol);
        }
    }

    get_command_output("distrobox", Some(args.as_slice()))
}

/// Builds the argument list for `distrobox create`, kept separate from the
/// spawning so it can be checked without a container engine. `nvidia` is
/// passed in rather than probed here for the same reason - the caller hands
/// in `is_nvidia()`, a test hands in a constant. An empty `home_path` or
/// `hostname` leaves the flag off entirely, so distrobox uses its default.
fn build_create_args(
    box_name: &str,
    image: &str,
    home_path: &str,
    hostname: &str,
    use_init: bool,
    volumes: &[String],
    nvidia: bool,
) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "create".into(),
        "-n".into(),
        box_name.into(),
        "-i".into(),
        image.into(),
        "-Y".into(),
    ];
    if nvidia {
        args.push("--nvidia".into());
    }
    if use_init {
        args.push("--init".into());
        args.push("--additional-packages".into());
        args.push("systemd".into());
    }
    if !home_path.is_empty() {
        args.push("--home".into());
        args.push(home_path.into());
    }
    if !hostname.is_empty() {
        args.push("--hostname".into());
        args.push(hostname.into());
    }
    for vol in volumes {
        args.push("--volume".into());
        args.push(vol.clone());
    }
    args
}

/// Streaming variant of `create_box`: every line of stdout and stderr is
/// forwarded to `tx` as soon as it is read, so the caller can render it
/// inside a `gtk::TextView` while the container is being built. The
/// function returns once `distrobox create` exits.
///
/// Each line is sent as a separate message; an empty line marks the end
/// of one stream (stdout then stderr). Two empty messages in a row signal
/// process exit. The exit code itself is *not* sent - callers that care
/// should use `create_box` or chain their own completion message after
/// this function returns.
///
/// The original `create_box` is preserved unchanged so other call sites
/// keep working. This is intentionally additive: the streaming path is
/// only useful during the *create* flow, which is the slowest command
/// BoxBuddy runs and the only one where progress feedback matters.
pub fn create_box_streaming(
    box_name: &str,
    image: &str,
    home_path: &str,
    hostname: &str,
    use_init: bool,
    volumes: &[String],
    tx: Sender<String>,
) {
    let args = build_create_args(
        box_name,
        image,
        home_path,
        hostname,
        use_init,
        volumes,
        is_nvidia(),
    );
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();

    let mut child = match Command::new("distrobox")
        .args(&arg_refs)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            let _ = tx.send(format!("Failed to spawn distrobox: {e}"));
            let _ = tx.send(String::new());
            let _ = tx.send(String::new());
            return;
        }
    };

    // Take the pipes out before we move the child into the join handle.
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    // Spawn a thread per stream. Each thread reads line-by-line and forwards
    // to the same channel; this lets us stream both streams concurrently
    // without ordering them on purpose (distrobox writes progress messages
    // to both, and the order is not meaningful to the user).
    let tx_out = tx.clone();
    let stdout_handle = stdout.map(|s| {
        std::thread::spawn(move || {
            let reader = BufReader::new(s);
            for line in reader.lines() {
                let Ok(line) = line else { break };
                if tx_out.send(line).is_err() {
                    break;
                }
            }
            let _ = tx_out.send(String::new());
        })
    });

    let tx_err = tx.clone();
    let stderr_handle = stderr.map(|s| {
        std::thread::spawn(move || {
            let reader = BufReader::new(s);
            for line in reader.lines() {
                let Ok(line) = line else { break };
                if tx_err.send(line).is_err() {
                    break;
                }
            }
            let _ = tx_err.send(String::new());
        })
    });

    // Wait for the process. We don't need the exit status here - the
    // completion of the two stream threads is enough to know the command
    // has finished writing output.
    let _ = child.wait();

    if let Some(h) = stdout_handle {
        let _ = h.join();
    }
    if let Some(h) = stderr_handle {
        let _ = h.join();
    }
}

/// Runs `distrobox-assemble` with the provided file.
pub fn assemble_box(ini_file: &str) -> String {
    let args = &["assemble", "create", "--file", ini_file];
    get_command_output("distrobox", Some(args))
}

/// One box section from a `distrobox.ini` file, parsed for the preview dialog.
#[derive(Debug, Clone)]
pub struct IniBoxSection {
    /// Section name (i.e. the box name).
    pub name: String,
    /// Container image URL or distro name (e.g. `ubuntu:24.04`).
    pub image: Option<String>,
    /// Additional packages declared inline (raw string, may contain commas).
    pub additional_packages: Option<String>,
    /// Custom home directory (if any).
    pub home: Option<String>,
    /// Whether the box will boot with `--init`.
    pub init: bool,
    /// Whether the box will use `--nvidia`.
    pub nvidia: bool,
    /// Every other key that was in the section, kept verbatim so the user
    /// can see what is being passed to `distrobox assemble create` unmodified.
    pub extra_keys: Vec<(String, String)>,
}

/// Parses a `distrobox.ini`-style file into a vector of box sections.
///
/// The format is a flat INI: one `[section]` header per box, with key=value
/// lines below it. We deliberately keep the parser tiny instead of pulling
/// in a new crate - `distrobox.ini` is a known, simple shape and the surface
/// we need to support is only `image`, `additional_packages`, `home`, `init`,
/// `nvidia`. Lines that do not parse into a section/header/key are skipped.
pub fn parse_assemble_ini(contents: &str) -> Vec<IniBoxSection> {
    let mut sections: Vec<IniBoxSection> = Vec::new();
    let mut current: Option<IniBoxSection> = None;

    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }

        if let Some(rest) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            // Flush the previous section before starting a new one.
            if let Some(sec) = current.take() {
                sections.push(sec);
            }
            current = Some(IniBoxSection {
                name: rest.trim().to_string(),
                image: None,
                additional_packages: None,
                home: None,
                init: false,
                nvidia: false,
                extra_keys: Vec::new(),
            });
            continue;
        }

        let Some(sec) = current.as_mut() else {
            continue;
        };

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim().trim_matches('"').to_string();

        match key {
            "image" => sec.image = Some(value),
            "additional_packages" => sec.additional_packages = Some(value),
            "home" => sec.home = Some(value),
            "init" => sec.init = parse_bool(&value),
            "nvidia" => sec.nvidia = parse_bool(&value),
            _ => sec.extra_keys.push((key.to_string(), value)),
        }
    }

    if let Some(sec) = current.take() {
        sections.push(sec);
    }

    sections
}

/// Accepts the few truthy spellings distrobox itself understands in its INI:
/// `true`, `yes`, `1`, `on` (case-insensitive). Anything else - including a
/// missing key - is treated as `false`.
fn parse_bool(value: &str) -> bool {
    matches!(value.to_lowercase().as_str(), "true" | "yes" | "1" | "on")
}

/// Grabs the list of available images via `distrobox create -C`.
/// Prepends the parsed distro name for sortability and readability.
/// Appends a little diamond if the image is already downloaded.
pub fn get_available_images_with_distro_name() -> Vec<String> {
    let existing_images = get_repository_list();
    let output = get_command_output("distrobox", Some(&["create", "--compatibility"]));

    let mut imgs: Vec<String> = Vec::new();

    for line in output.split('\n') {
        if line.is_empty() || line == "Images" {
            continue;
        }

        let distro = try_parse_distro_name_from_url(line);
        let mut pretty_line = if distro == "zunknown" {
            format!("unknown - {line}")
        } else {
            format!("{distro} - {line}")
        };

        if existing_images.contains(&line.to_string()) {
            pretty_line = format!("{pretty_line} ✦ ");
        }

        imgs.push(pretty_line);
    }

    imgs.sort();

    imgs
}

/// Lists desktop files available in a distrobox, for the View Applications pop-up
pub fn get_apps_in_box(box_name: &str) -> Vec<DBoxApp> {
    let mut apps: Vec<DBoxApp> = Vec::new();

    // get list of host apps to check against afterwards
    let host_apps = get_host_desktop_files();

    let desktop_files = get_command_output(
        "distrobox",
        Some(&[
            "enter",
            box_name,
            "--",
            "bash",
            "-c",
            "grep --files-without-match \"NoDisplay=true\" /usr/share/applications/*.desktop",
        ]),
    );

    for line in desktop_files.split('\n') {
        if line.is_empty() || line.contains("No such file") || !line.starts_with("/usr/share") {
            continue;
        }

        let desktop_file_contents =
            get_command_output("distrobox", Some(&["enter", box_name, "--", "cat", line]));

        let mut pieces: [String; 3] = [String::new(), String::new(), String::new()];

        for df_line in desktop_file_contents.split('\n') {
            if pieces[0].is_empty() && df_line.starts_with("Name=") {
                if let Some(l) = df_line.strip_prefix("Name=") {
                    pieces[0] = l.to_string();
                }
            } else if pieces[1].is_empty() && df_line.starts_with("Exec=") {
                if let Some(l) = df_line.strip_prefix("Exec=") {
                    pieces[1] = l.to_string();
                }
            } else if pieces[2].is_empty() && df_line.starts_with("Icon=") {
                if let Some(l) = df_line.strip_prefix("Icon=") {
                    pieces[2] = l.to_string();
                }
            }
        }

        if pieces[0].is_empty() || pieces[1].is_empty() {
            continue;
        }

        // figure out if this exists on the host so we can show remove btn instead
        let desktop_file_name = line
            .replace("/usr/share/applications/", "")
            .replace(".desktop", "");

        let host_desktop_name = format!("{box_name}-{desktop_file_name}.desktop");

        let app = DBoxApp {
            name: pieces[0].clone(),
            exec_name: pieces[1]
                .replace("%F", "")
                .replace("%U", "")
                .trim()
                .to_owned(),
            icon: pieces[2].clone(),
            desktop_file: desktop_file_name,
            is_on_host: host_apps.contains(&host_desktop_name),
        };

        apps.push(app);
    }

    apps
}

pub fn get_binaries_exported_from_box(box_name: &str) -> Vec<String> {
    let output = get_command_output(
        "distrobox",
        Some(&[
            "enter",
            box_name,
            "--",
            "distrobox-export",
            "--list-binaries",
        ]),
    );

    let mut binaries = Vec::<String>::new();

    for line in output.split('\n') {
        if line.is_empty() || !line.contains('|') {
            continue;
        }

        let (bin_path, exported_path) = match line.find('|') {
            Some(index) => (&line[..index], &line[index + 1..]),
            None => ("", ""),
        };

        if !exported_path.is_empty() {
            binaries.push(exported_path.trim().to_string());
        }
    }

    binaries
}

pub fn remove_exported_binary_from_box(box_name: &str, binary: &str) {
    let _ = run_command(
        "distrobox",
        Some(&[
            "enter",
            box_name,
            "--",
            "distrobox-export",
            "--bin",
            binary,
            "-d",
        ]),
    );
}

pub fn stop_box(box_name: &str) {
    let _ = run_command("distrobox", Some(&["stop", box_name, "--yes"]));
}

/// Gets count of boxes, used to move the active page on the Notebook to the newest
/// box after creation.
pub fn get_number_of_boxes() -> u32 {
    // Counting the lines of `distrobox list` ourselves would count the fragments
    // described in `is_short_container_id` too, and the count is used to pick a
    // tab, so it has to agree with the list the tabs were built from.
    u32::try_from(get_all_distroboxes().len()).unwrap_or(u32::MAX)
}

/// Runs the `distrobox enter NAME -- sudo <manager> install PATH` command
/// in a terminal so the user can confirm the `sudo` prompt. Used by both
/// the `.deb` and `.rpm` install paths - the only thing that varies is
/// which package manager we ask for.
fn run_install_in_terminal(box_name: &str, file_path: &str, manager: &str) {
    let (term, sep, term_is_flatpak) = get_terminal_and_separator_arg();

    if is_flatpak() {
        if term_is_flatpak {
            Command::new("flatpak-spawn")
                .arg("--host")
                .arg("flatpak")
                .arg("run")
                .arg(term)
                .arg(sep)
                .arg("distrobox")
                .arg("enter")
                .arg(box_name)
                .arg("--")
                .arg("sudo")
                .arg(manager)
                .arg("install")
                .arg(file_path)
                .spawn()
                .unwrap();
        } else {
            Command::new("flatpak-spawn")
                .arg("--host")
                .arg(term)
                .arg(sep)
                .arg("distrobox")
                .arg("enter")
                .arg(box_name)
                .arg("--")
                .arg("sudo")
                .arg(manager)
                .arg("install")
                .arg(file_path)
                .spawn()
                .unwrap();
        }
    } else if term_is_flatpak {
        Command::new("flatpak")
            .arg("run")
            .arg(term)
            .arg(sep)
            .arg("distrobox")
            .arg("enter")
            .arg(box_name)
            .arg("--")
            .arg("sudo")
            .arg(manager)
            .arg("install")
            .arg(file_path)
            .spawn()
            .unwrap();
    } else {
        Command::new(term)
            .arg(sep)
            .arg("distrobox")
            .arg("enter")
            .arg(box_name)
            .arg("--")
            .arg("sudo")
            .arg(manager)
            .arg("install")
            .arg(file_path)
            .spawn()
            .unwrap();
    }
}

/// Tries to install a .deb file in the box using the package manager we
/// infer from the box's image. Falls back to `apt` for unknown images
/// because most deb-shaped distros in distrobox's supported list do use
/// it; the user gets a visible error in the terminal if that guess is
/// wrong.
pub fn install_deb_in_box(box_name: String, image: String, file_path: String) {
    let manager = match crate::utils::detect_pkg_manager(&image) {
        Some(crate::utils::PkgManager::Apt) => "apt",
        // .deb is not the native package format for non-apt distros;
        // refusing here would be safer than producing an apt-only error,
        // but the old behaviour was to always try apt, so we keep that
        // as a safe default and surface the failure in the terminal
        // where the user can read it.
        _ => "apt",
    };
    run_install_in_terminal(&box_name, &file_path, manager);
}

/// Tries to install a .rpm file in the box using the package manager we
/// infer from the box's image. Detects both `dnf` (Fedora / RHEL clones)
/// and `zypper` (openSUSE) and falls back to `dnf` for unknown images.
/// Like the `.deb` path, the actual command runs in a terminal so the
/// user can confirm the `sudo` prompt.
pub fn install_rpm_in_box(box_name: String, image: String, file_path: String) {
    let manager = match crate::utils::detect_pkg_manager(&image) {
        Some(crate::utils::PkgManager::Zypper) => "zypper",
        Some(crate::utils::PkgManager::Dnf) => "dnf",
        _ => "dnf",
    };
    run_install_in_terminal(&box_name, &file_path, manager);
}

pub fn clone_box(box_to_clone: &str, new_name: &str) -> String {
    stop_box(box_to_clone);

    get_command_output(
        "distrobox",
        Some(&["create", "--clone", box_to_clone, "--name", new_name]),
    )
}

pub fn upgrade_all_boxes() {
    let (term, sep, term_is_flatpak) = get_terminal_and_separator_arg();
    let command = format!("distrobox-upgrade --all");

    if is_flatpak() {
        if term_is_flatpak {
            Command::new("flatpak-spawn")
                .arg("--host")
                .arg("flatpak")
                .arg("run")
                .arg(term)
                .arg(sep)
                .arg("bash")
                .arg("-c")
                .arg(&command)
                .spawn()
                .unwrap();
        } else {
            Command::new("flatpak-spawn")
                .arg("--host")
                .arg(term)
                .arg(sep)
                .arg("bash")
                .arg("-c")
                .arg(&command)
                .spawn()
                .unwrap();
        }
    } else {
        if term_is_flatpak {
            Command::new("flatpak")
                .arg("run")
                .arg(term)
                .arg(sep)
                .arg("bash")
                .arg("-c")
                .arg(&command)
                .spawn()
                .unwrap();
        } else {
            Command::new(term)
                .arg(sep)
                .arg("bash")
                .arg("-c")
                .arg(&command)
                .spawn()
                .unwrap();
        }
    }
}

#[cfg(test)]
mod stream_tests {
    use super::{build_create_args, create_box_streaming};

    #[test]
    fn minimal_args_leave_optional_flags_off() {
        let args = build_create_args(
            "mybox",
            "docker.io/library/ubuntu:latest",
            "",
            "",
            false,
            &[],
            false,
        );
        assert_eq!(
            args,
            vec![
                "create",
                "-n",
                "mybox",
                "-i",
                "docker.io/library/ubuntu:latest",
                "-Y"
            ]
        );
    }

    #[test]
    fn every_option_lands_in_the_args() {
        let vols = vec!["/a:/a".to_string(), "/b:/b".to_string()];
        let args = build_create_args("dev", "img", "/home/me/box", "devhost", true, &vols, true);
        assert!(args.windows(2).any(|w| w == ["--home", "/home/me/box"]));
        assert!(args.windows(2).any(|w| w == ["--hostname", "devhost"]));
        assert!(args
            .windows(3)
            .any(|w| w == ["--init", "--additional-packages", "systemd"]));
        assert!(args.contains(&"--nvidia".to_string()));
        assert_eq!(args.iter().filter(|a| *a == "--volume").count(), 2);
        assert!(args.windows(2).any(|w| w == ["--volume", "/a:/a"]));
        assert!(args.windows(2).any(|w| w == ["--volume", "/b:/b"]));
    }

    /// End-to-end: actually create a box through the streaming function,
    /// prove lines flow and the box appears, then remove it. Ignored by
    /// default because it needs a working distrobox and pulls an image;
    /// run with `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn streaming_create_emits_lines_and_makes_a_box() {
        use super::{delete_box, get_all_distroboxes};
        use std::sync::mpsc::channel;

        let name = "bb-stream-selftest";
        let _ = delete_box(name);

        let (tx, rx) = channel();
        create_box_streaming(
            name,
            "docker.io/library/ubuntu:latest",
            "",
            "",
            false,
            &[],
            tx,
        );

        let lines: Vec<String> = rx.iter().collect();
        let non_empty = lines.iter().filter(|l| !l.is_empty()).count();
        assert!(non_empty > 0, "expected some output lines, got none");

        let exists = get_all_distroboxes().iter().any(|b| b.name == name);
        let _ = delete_box(name);
        assert!(exists, "streaming create did not produce a listable box");
    }
}

#[cfg(test)]
mod ini_preview_tests {
    use super::parse_assemble_ini;

    #[test]
    fn parses_a_single_section() {
        let ini = "[dev]\nimage=docker.io/library/ubuntu:24.04\nadditional_packages=\"git vim\"\ninit=true\n";
        let s = parse_assemble_ini(ini);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].name, "dev");
        assert_eq!(
            s[0].image.as_deref(),
            Some("docker.io/library/ubuntu:24.04")
        );
        assert_eq!(s[0].additional_packages.as_deref(), Some("git vim"));
        assert!(s[0].init);
        assert!(!s[0].nvidia);
        assert!(s[0].extra_keys.is_empty());
    }

    #[test]
    fn parses_multiple_sections() {
        let ini = "[a]\nimage=alpine\n\n[b]\nimage=fedora\nnvidia=true\n";
        let s = parse_assemble_ini(ini);
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].name, "a");
        assert_eq!(s[1].name, "b");
        assert!(s[1].nvidia);
    }

    /// The whole point of the preview: keys BoxBuddy has no field for must
    /// still be captured, so the dialog can show them rather than hiding
    /// what the file will actually do.
    #[test]
    fn keeps_unknown_keys_verbatim() {
        let ini = "[x]\nimage=ubuntu\npull=true\ninit_hooks=curl example.com | sh\n";
        let s = parse_assemble_ini(ini);
        assert_eq!(s[0].extra_keys.len(), 2);
        assert!(s[0]
            .extra_keys
            .contains(&("pull".to_string(), "true".to_string())));
        assert!(s[0].extra_keys.contains(&(
            "init_hooks".to_string(),
            "curl example.com | sh".to_string()
        )));
    }

    #[test]
    fn skips_comments_blank_lines_and_junk() {
        let ini = "# a comment\n; another\n[d]\n\nnonsense-without-equals\nimage=debian\n";
        let s = parse_assemble_ini(ini);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].image.as_deref(), Some("debian"));
        assert!(s[0].extra_keys.is_empty());
    }

    #[test]
    fn recognises_various_truthy_spellings() {
        for v in ["true", "yes", "1", "on", "TRUE", "On"] {
            let ini = format!("[s]\nimage=i\ninit={v}\n");
            assert!(parse_assemble_ini(&ini)[0].init, "init={v} should be true");
        }
        for v in ["false", "no", "0", "off", ""] {
            let ini = format!("[s]\nimage=i\ninit={v}\n");
            assert!(
                !parse_assemble_ini(&ini)[0].init,
                "init={v} should be false"
            );
        }
    }

    #[test]
    fn keys_before_any_section_are_ignored() {
        let ini = "image=orphan\n[real]\nimage=ubuntu\n";
        let s = parse_assemble_ini(ini);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].name, "real");
    }

    #[test]
    fn empty_input_yields_no_sections() {
        assert!(parse_assemble_ini("").is_empty());
        assert!(parse_assemble_ini("# just a comment\n").is_empty());
    }
}
