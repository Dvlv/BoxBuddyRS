use crate::utils::{
    detect_pkg_manager, get_command_output, get_container_runtime, get_host_desktop_files,
    get_repository_list, get_terminal_and_separator_arg, is_flatpak, is_nvidia, run_command,
    PkgManager,
};
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;

/// Struct representing a distrobox installed on the user's machine
#[derive(Clone)]
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

/// The in-container path of an application's desktop file, built from the
/// desktop-file id that `get_apps_in_box` records (the file's basename).
/// distrobox reads a box's apps from `/usr/share/applications`.
fn desktop_file_path(desktop_file: &str) -> String {
    format!("/usr/share/applications/{desktop_file}.desktop")
}

/// Exports an application's desktop file from a box to the host menu.
///
/// The app is identified by its desktop-file id, not its display name. `--app`
/// matches against the desktop file, and a display name can be empty, repeated
/// across apps, or match several files - which is how a single click could
/// export more than the one app. Handing distrobox the exact file path exports
/// precisely that app, and keeps export in step with how the host copy is
/// detected (`{box}-{id}.desktop`) and removed.
///
/// `label` overrides the text distrobox puts after the app's name in the menu
/// (`--export-label`); `None` leaves distrobox's own default, `(on <box>)`.
pub fn export_app_from_box(desktop_file: &str, box_name: &str, label: Option<&str>) -> String {
    let app_path = desktop_file_path(desktop_file);
    let mut args: Vec<&str> = vec![
        "enter",
        box_name,
        "--",
        "distrobox-export",
        "--app",
        app_path.as_str(),
    ];
    if let Some(label) = label {
        args.push("--export-label");
        args.push(label);
    }
    get_command_output("distrobox", Some(&args))
}

/// Unexports an application's desktop file from the host. Identified by the same
/// desktop-file id used to export it, so removal always targets the right app.
pub fn remove_app_from_host(desktop_file: &str, box_name: &str) -> String {
    let app_path = desktop_file_path(desktop_file);
    get_command_output(
        "distrobox",
        Some(&[
            "enter",
            box_name,
            "--",
            "distrobox-export",
            "--app",
            &app_path,
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
    stream_distrobox(&tx, &arg_refs);

    // `distrobox create` only writes the container's configuration. The
    // container is actually built the first time it is entered, which is when
    // distrobox prints "Starting container", "Installing basic packages" and
    // the rest of the setup. That used to scroll past in a terminal we opened
    // afterwards; trigger it here with a no-op enter instead, so the same
    // dialog shows the setup too.
    stream_distrobox(&tx, &["enter", box_name, "--", "true"]);
}

/// Spawns one `distrobox` invocation and forwards every line of its stdout and
/// stderr to `tx` as it arrives, ending each stream with an empty line so the
/// dialog can tell a stream has finished. Returns once the process exits.
///
/// stdout and stderr are read on their own threads and interleaved on purpose:
/// distrobox writes progress to both and the order is not meaningful.
fn stream_distrobox(tx: &Sender<String>, args: &[&str]) {
    let mut child = match Command::new("distrobox")
        .args(args)
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

    // Wait for the process. We don't need the exit status here - the two
    // stream threads finishing is enough to know it has stopped writing.
    let _ = child.wait();

    if let Some(h) = stdout_handle {
        let _ = h.join();
    }
    if let Some(h) = stderr_handle {
        let _ = h.join();
    }
}

/// Builds the body of a distrobox-assemble `.ini` file from the fields of the
/// generator form. Kept here, and pure, so both the live preview and the save
/// path render identical text from one place and it can be unit-tested.
/// `section` and `image` are assumed already validated as non-empty by the
/// caller.
pub fn build_assemble_ini(
    section: &str,
    image: &str,
    packages: &str,
    home: &str,
    init: bool,
    nvidia: bool,
) -> String {
    let mut body = String::new();
    body.push_str(&format!("[{section}]\n"));
    body.push_str(&format!("image={image}\n"));
    if !packages.trim().is_empty() {
        body.push_str(&format!("additional_packages=\"{packages}\"\n"));
    }
    if !home.trim().is_empty() {
        body.push_str(&format!("home={home}\n"));
    }
    if init {
        body.push_str("init=true\n");
    }
    if nvidia {
        body.push_str("nvidia=true\n");
    }
    body
}

/// Streaming variant of `upgrade_box`: runs `distrobox upgrade <box>` and
/// forwards its output to `tx`, so the upgrade can be shown inside the app
/// rather than a spawned terminal. `distrobox upgrade` drives the package
/// manager non-interactively (the container has passwordless sudo), so nothing
/// waits on input the way an interactive install would.
pub fn upgrade_box_streaming(box_name: &str, tx: Sender<String>) {
    stream_distrobox(&tx, &["upgrade", box_name]);
}

/// Streaming variant of `upgrade_all_boxes`: `distrobox upgrade --all`, streamed
/// the same way.
pub fn upgrade_all_boxes_streaming(tx: Sender<String>) {
    stream_distrobox(&tx, &["upgrade", "--all"]);
}

/// Runs `distrobox-assemble` with the provided file.
pub fn assemble_box(ini_file: &str) -> String {
    let args = &["assemble", "create", "--file", ini_file];
    get_command_output("distrobox", Some(args))
}

/// Parses a `distrobox.ini`-style file into its box sections: the section
/// name and every `key=value` it sets, verbatim and in file order. The
/// format is a flat INI with one `[section]` per box, so a tiny parser does
/// instead of a crate; comments, blank lines and lines that are neither a
/// header nor a key are skipped, as are keys before the first header.
pub fn parse_assemble_ini(contents: &str) -> Vec<(String, Vec<(String, String)>)> {
    let mut sections: Vec<(String, Vec<(String, String)>)> = Vec::new();

    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }

        if let Some(name) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            sections.push((name.trim().to_string(), Vec::new()));
        } else if let (Some((key, value)), Some((_, keys))) =
            (line.split_once('='), sections.last_mut())
        {
            let value = value.trim().trim_matches('"');
            keys.push((key.trim().to_string(), value.to_string()));
        }
    }

    sections
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

        let host_desktop_name = exported_desktop_file_name(box_name, &desktop_file_name);

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

/// Name of the desktop file `distrobox-export` writes on the host for an app.
pub fn exported_desktop_file_name(box_name: &str, desktop_file: &str) -> String {
    format!("{box_name}-{desktop_file}.desktop")
}

/// Whether the app's desktop file is currently in the host's menu.
pub fn is_app_exported(box_name: &str, desktop_file: &str) -> bool {
    get_host_desktop_files().contains(&exported_desktop_file_name(box_name, desktop_file))
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

/// Exports `bin_path` from inside `box_name` to the host terminal's
/// `~/.local/bin` via `distrobox-export --bin`. Without an explicit
/// `--export-path`, distrobox defaults to
/// `${DISTROBOX_EXPORT_PATH:-${host_home}/.local/bin}` which is what the
/// rest of BoxBuddy assumes. Passing the literal string `~/.local/bin`
/// would not work because no shell expands it.
pub fn export_binary_from_box(box_name: &str, bin_path: &str) {
    let _ = run_command(
        "distrobox",
        Some(&[
            "enter",
            box_name,
            "--",
            "distrobox-export",
            "--bin",
            bin_path,
        ]),
    );
}

/// Resolves `name` inside `box_name` via `distrobox enter … bash -lc 'command -v
/// -- <name>'`. Returns the trimmed absolute path when the box has it, None
/// otherwise. Used by the "Add Command to Terminal" UI to confirm the box
/// really has the command before asking anything about the host side.
pub fn box_command_path(box_name: &str, name: &str) -> Option<String> {
    if !valid_command_name(name) {
        return None;
    }
    let script = format!("command -v -- {}", name);
    let out = get_command_output(
        "distrobox",
        Some(&["enter", box_name, "--", "bash", "-lc", &script]),
    );
    let trimmed = out.trim();
    if trimmed.starts_with('/') {
        Some(trimmed.to_string())
    } else {
        None
    }
}

/// Description of what the host already has on the path for a given command
/// name. The "Add Command to Terminal" dialog needs to know whether to plain-
/// export (nothing in the way), warn the user (a host binary with the same
/// name) or fold the new box into an existing dispatcher.
pub struct HostCommandState {
    /// Other host-side paths matching the name. Does NOT include
    /// `$HOME/.local/bin/<name>` when that file is a dispatcher or a
    /// distrobox-export wrapper, because the chooser is about to replace it.
    pub host_paths: Vec<String>,
    /// Box name extracted from a distrobox-export wrapper sitting at
    /// `$HOME/.local/bin/<name>`, if any.
    pub wrapper_box: Option<String>,
    /// `(host, boxes)` parsed from a BoxBuddy dispatcher at
    /// `$HOME/.local/bin/<name>`, if any.
    pub dispatcher: Option<(Option<String>, Vec<String>)>,
}

/// Looks at the host side for a command called `name`. Sees whether the host
/// has its own `name` binary, a distrobox wrapper sitting in
/// `$HOME/.local/bin/<name>` or an existing BoxBuddy dispatcher. Parsing the
/// wrapper is defensive: if no box name can be extracted the file is treated
/// as a plain host binary (wrapper_box stays None and the local path is
/// included in host_paths).
pub fn host_command_conflicts(name: &str) -> HostCommandState {
    let mut state = HostCommandState {
        host_paths: vec![],
        wrapper_box: None,
        dispatcher: None,
    };
    if !valid_command_name(name) {
        return state;
    }

    // One login shell answers everything: where $HOME is, what the user's PATH
    // resolves the name to, whether ~/.local/bin already holds a file of that
    // name, and what is in it. Sections are split by a form-feed line, which
    // no path or script line contains.
    // Each separator carries its own leading newline: a section that produces
    // no output (no match on PATH, no local file) would otherwise not end in a
    // newline and the sections would shift by one.
    let probe = format!(
        "printf '%s' \"$HOME\"; printf '\\n\\f\\n'; \
         type -aP -- {name} 2>/dev/null; printf '\\n\\f\\n'; \
         test -e \"$HOME/.local/bin/{name}\" && echo yes; printf '\\n\\f\\n'; \
         cat -- \"$HOME/.local/bin/{name}\" 2>/dev/null"
    );
    let out = get_command_output("bash", Some(&["-lc", &probe]));
    let mut parts = out.split("\n\u{c}\n");
    let home = parts.next().unwrap_or_default().trim().to_string();
    let paths = parts.next().unwrap_or_default();
    let local_present = !parts.next().unwrap_or_default().trim().is_empty();
    let local_file = parts.next().unwrap_or_default();

    let local_path = format!("{home}/.local/bin/{name}");
    let is_dispatcher = local_file
        .lines()
        .any(|l| l.starts_with("# boxbuddy-dispatcher:"));
    let is_wrapper = !is_dispatcher && local_file.contains("# distrobox_binary");

    if is_dispatcher {
        state.dispatcher = parse_dispatcher_marker(local_file);
    } else if is_wrapper {
        state.wrapper_box = parse_distrobox_wrapper_box(local_file);
    }

    // What the chooser is about to replace is described by the dispatcher /
    // wrapper_box fields, so it must not also be listed as a rival host
    // binary. Everything else the shell resolves is a real clash.
    let replacing_local = is_dispatcher || state.wrapper_box.is_some();
    for p in paths.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if replacing_local && p == local_path {
            continue;
        }
        state.host_paths.push(p.to_string());
    }

    // `type -aP` only sees what is on PATH. With ~/.local/bin missing from it,
    // an existing file there would go unnoticed and a plain export would
    // silently overwrite it.
    if local_present && !replacing_local && !state.host_paths.contains(&local_path) {
        state.host_paths.push(local_path);
    }

    state
}

/// Pulls the box name out of a `distrobox-export --bin` wrapper script.
/// Tries, in order: (a) the `# name: <container>` comment distrobox itself
/// writes, then (b) the bare `-n <container>` argument on the
/// `distrobox-enter` exec line. Returns None if neither matches; the caller
/// then treats the file as a plain host binary.
fn parse_distrobox_wrapper_box(content: &str) -> Option<String> {
    // (a) The `# name: <container>` comment distrobox itself writes is the
    // authoritative source.
    for line in content.lines() {
        if let Some(rest) = line.trim_start().strip_prefix("# name:") {
            let name = rest.trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }

    // (b) Failing that, the bare `-n <container>` argument on the
    // distrobox-enter exec line (space-separated, NOT `--name`).
    for line in content.lines() {
        if !line.contains("distrobox-enter") {
            continue;
        }
        let mut tokens = line.split_whitespace();
        while let Some(tok) = tokens.next() {
            if tok == "-n" {
                if let Some(name) = tokens.next() {
                    if !name.is_empty() {
                        return Some(name.to_string());
                    }
                }
            }
        }
    }

    None
}

/// Validates a command name the user typed in the "Add Command" entry. Names
/// must be non-empty, contain only `[A-Za-z0-9._+-]` and not start with `-`
/// or `.`. Used by the UI before anything else to keep shell-escaping
/// corners closed.
pub fn valid_command_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let mut chars = name.chars();
    let first = chars.next().unwrap();
    if first == '-' || first == '.' {
        return false;
    }
    name.chars()
        .all(|c| matches!(c, 'A'..='Z' | 'a'..='z' | '0'..='9' | '.' | '_' | '+' | '-'))
}

/// Builds the bash source for a dispatcher. Pure - no I/O - so the same body
/// can be parsed by `parse_dispatcher_marker`, written to disk by
/// `write_dispatcher` and inspected by tests.
///
/// Behaviour:
/// * `BOXBUDDY_DISPATCH=host` runs the host binary (errors if no host).
/// * `BOXBUDDY_DISPATCH=<box>` runs that box via `distrobox enter`.
/// * Any other env value errors out without prompting.
/// * When stderr is a tty and `/dev/tty` is readable, the user gets a
///   numbered menu on stderr and `read` happens from `/dev/tty`.
/// * Otherwise the first target (host when present, else first box) runs
///   without asking.
///
/// A host path containing whitespace cannot round-trip through the marker
/// line (tokens are whitespace-separated); runtime quoting of the
/// dispatched command is unaffected.
pub fn dispatcher_script(name: &str, host: Option<&str>, boxes: &[String]) -> String {
    // One `# name:` line per box, so `distrobox-export --list-binaries` run in
    // any of the target boxes still finds this command, and
    // `distrobox-export --bin … --delete` still recognises the file. Without
    // them distrobox quietly loses sight of a command it had exported.
    let mut markers = String::from("# distrobox_binary\n");
    for b in boxes {
        markers.push_str(&format!("# name: {b}\n"));
    }
    markers.push_str(&format!(
        "# boxbuddy-dispatcher: command={} host={} boxes={}",
        name,
        host.unwrap_or(""),
        boxes.join(",")
    ));

    let boxes_arr = boxes
        .iter()
        .map(|b| bash_quote(b))
        .collect::<Vec<_>>()
        .join(" ");

    DISPATCHER_TEMPLATE
        .replace("@MARKERS@", &markers)
        .replace(
            "@HOST@",
            &host.map(bash_quote).unwrap_or_else(|| "''".into()),
        )
        .replace("@BOXES@", &boxes_arr)
        .replace("@NAME@", &bash_quote(name))
}

/// The dispatcher itself. Kept as one template rather than assembled line by
/// line: every generated file then has the same shape whatever the targets
/// are, so reading one tells you how all of them behave. `host` is the first
/// entry when it is set; a box literally named `host` would be shadowed by it.
const DISPATCHER_TEMPLATE: &str = r#"#!/usr/bin/env bash
@MARKERS@
# BoxBuddy regenerates this file; manual edits will be lost.
HOST=@HOST@
BOXES=(@BOXES@)
NAME=@NAME@

TARGETS=()
[ -n "$HOST" ] && TARGETS+=("host")
TARGETS+=("${BOXES[@]}")

run_target() {
	target=$1
	shift
	if [ "$target" = "host" ]; then
		exec "$HOST" "$@"
	fi
	exec distrobox enter "$target" -- "$NAME" "$@"
}

if [ ${#TARGETS[@]} -eq 0 ]; then
	echo "$NAME: no targets configured" >&2
	exit 2
fi

# Scripts and pipes must never block on a prompt: BOXBUDDY_DISPATCH picks a
# target outright, and with no terminal the first target is used silently.
if [ -n "${BOXBUDDY_DISPATCH:-}" ]; then
	for t in "${TARGETS[@]}"; do
		[ "$BOXBUDDY_DISPATCH" = "$t" ] && run_target "$t" "$@"
	done
	echo "$NAME: unknown BOXBUDDY_DISPATCH target: $BOXBUDDY_DISPATCH" >&2
	exit 2
fi

INDEX=1
if [ -t 2 ] && [ -r /dev/tty ]; then
	echo "Run $NAME with:" >&2
	i=1
	for t in "${TARGETS[@]}"; do
		if [ "$t" = "host" ]; then
			echo "  $i: host ($HOST)" >&2
		else
			echo "  $i: $t" >&2
		fi
		i=$((i + 1))
	done
	printf 'Run %s from [1]: ' "$NAME" >&2
	read -r CHOICE < /dev/tty || CHOICE=1
	[ -n "$CHOICE" ] || CHOICE=1
	case $CHOICE in
	'' | *[!0-9]*)
		echo "$NAME: invalid choice: $CHOICE" >&2
		exit 2
		;;
	esac
	INDEX=$CHOICE
fi

if [ "$INDEX" -lt 1 ] || [ "$INDEX" -gt ${#TARGETS[@]} ]; then
	echo "$NAME: choice out of range" >&2
	exit 2
fi
run_target "${TARGETS[$((INDEX - 1))]}" "$@"
"#;

/// Single-quote-safe bash quoting. A name like `it's` becomes `'it'\''s'`,
/// which is safe to embed between `()` in `BOXES=(...)` and between `=` in
/// `HOST=...`.
fn bash_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

/// Parses the marker line emitted by `dispatcher_script`. Returns `(host,
/// boxes)` where `host` is None when the marker says `host=` (no host
/// target). Returns None when the content has no marker.
///
/// A host path containing whitespace cannot round-trip through the marker
/// line (tokens are whitespace-separated); runtime quoting of the
/// dispatched command is unaffected.
pub fn parse_dispatcher_marker(content: &str) -> Option<(Option<String>, Vec<String>)> {
    for line in content.lines() {
        let Some(rest) = line.strip_prefix("# boxbuddy-dispatcher:") else {
            continue;
        };
        let mut host: Option<String> = None;
        let mut boxes: Vec<String> = Vec::new();
        for token in rest.split_whitespace() {
            if let Some(h) = token.strip_prefix("host=") {
                host = if h.is_empty() {
                    None
                } else {
                    Some(h.to_string())
                };
            } else if let Some(b) = token.strip_prefix("boxes=") {
                if !b.is_empty() {
                    boxes = b.split(',').map(String::from).collect();
                }
            }
        }
        return Some((host, boxes));
    }
    None
}

/// Writes the dispatcher to `$HOME/.local/bin/<name>` on the host via
/// `run_command`. The body is passed through a quoted heredoc so bash does
/// not touch it; the heredoc tag (`BBDISPATCH`) cannot appear inside a
/// validated command name nor inside any of the lines `dispatcher_script`
/// emits.
pub fn write_dispatcher(name: &str, host: Option<&str>, boxes: &[String]) {
    if !valid_command_name(name) {
        return;
    }
    let content = dispatcher_script(name, host, boxes);
    let mut script = String::new();
    script.push_str("mkdir -p \"$HOME/.local/bin\" && cat > \"$HOME/.local/bin/");
    script.push_str(name);
    script.push_str("\" <<'BBDISPATCH'\n");
    script.push_str(&content);
    script.push_str("BBDISPATCH\n");
    script.push_str("chmod +x \"$HOME/.local/bin/");
    script.push_str(name);
    script.push('"');
    let _ = run_command("bash", Some(&["-c", &script]));
}

/// Lists BoxBuddy dispatchers under `$HOME/.local/bin` whose marker points
/// at `box_name`. Returns `(command_name, host, boxes)` triples; the command
/// name is the file name, since that is what the shell resolves on PATH.
pub fn list_dispatchers_for_box(box_name: &str) -> Vec<(String, Option<String>, Vec<String>)> {
    let script = "grep -sl '^# boxbuddy-dispatcher:' \"$HOME/.local/bin/\"* 2>/dev/null";
    let out = get_command_output("bash", Some(&["-c", script]));
    let mut result = Vec::new();
    for line in out.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let content = get_command_output(
            "bash",
            Some(&["-c", &format!("cat -- {}", bash_quote(line))]),
        );
        let Some((host, boxes)) = parse_dispatcher_marker(&content) else {
            continue;
        };
        if !boxes.iter().any(|b| b == box_name) {
            continue;
        }
        let Some(cmd_name) = std::path::Path::new(line)
            .file_name()
            .and_then(|s| s.to_str())
        else {
            continue;
        };
        result.push((cmd_name.to_string(), host, boxes));
    }
    result
}

/// Deletes the dispatcher at `$HOME/.local/bin/<name>`. A no-op when the
/// name fails validation; otherwise `rm -f` so the call is idempotent.
pub fn remove_dispatcher(name: &str) {
    if !valid_command_name(name) {
        return;
    }
    let script = format!("rm -f -- \"$HOME/.local/bin/{}\"", name);
    let _ = run_command("bash", Some(&["-c", &script]));
}

pub fn stop_box(box_name: &str) {
    let _ = run_command("distrobox", Some(&["stop", box_name, "--yes"]));
}

/// Starts a stopped container via the underlying container engine.
/// `distrobox start` does not exist; entering the box would start it too,
/// but that spawns a shell we would immediately have to throw away, so
/// asking the runtime directly is the quiet way to bring it back up.
pub fn start_box(box_name: &str) {
    let runtime = get_container_runtime();
    let _ = run_command(&runtime, Some(&["start", box_name]));
}

/// Stops the box and brings it straight back up. Both halves are quiet,
/// non-interactive commands, so this runs inline like Stop and Start do.
pub fn reboot_box(box_name: &str) {
    stop_box(box_name);
    start_box(box_name);
}

/// Gets count of boxes, used to move the active page on the Notebook to the newest
/// box after creation.
pub fn get_number_of_boxes() -> u32 {
    // Counting the lines of `distrobox list` ourselves would count the fragments
    // described in `is_short_container_id` too, and the count is used to pick a
    // tab, so it has to agree with the list the tabs were built from.
    u32::try_from(get_all_distroboxes().len()).unwrap_or(u32::MAX)
}

/// Runs `distrobox enter NAME -- sudo <manager> <args...>` in a terminal so
/// the user can answer the `sudo` prompt and the manager's own confirmation.
/// Used by the `.deb`/`.rpm` install paths and by uninstall - only the
/// manager and its arguments differ. The pieces are passed as separate
/// arguments, never through a shell, so nothing in them is interpreted.
fn run_pkg_command_in_terminal(box_name: &str, manager: &str, args: &[&str]) {
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
                .args(args)
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
                .args(args)
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
            .args(args)
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
            .args(args)
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
    let manager = match detect_pkg_manager(&image) {
        Some(PkgManager::Apt) => "apt",
        // .deb is not the native package format for non-apt distros;
        // refusing here would be safer than producing an apt-only error,
        // but the old behaviour was to always try apt, so we keep that
        // as a safe default and surface the failure in the terminal
        // where the user can read it.
        _ => "apt",
    };
    run_pkg_command_in_terminal(&box_name, manager, &["install", &file_path]);
}

/// Tries to install a .rpm file in the box using the package manager we
/// infer from the box's image. Detects both `dnf` (Fedora / RHEL clones)
/// and `zypper` (openSUSE) and falls back to `dnf` for unknown images.
/// Like the `.deb` path, the actual command runs in a terminal so the
/// user can confirm the `sudo` prompt.
pub fn install_rpm_in_box(box_name: String, image: String, file_path: String) {
    let manager = match detect_pkg_manager(&image) {
        Some(PkgManager::Zypper) => "zypper",
        Some(PkgManager::Dnf) => "dnf",
        _ => "dnf",
    };
    run_pkg_command_in_terminal(&box_name, manager, &["install", &file_path]);
}

pub fn clone_box(box_to_clone: &str, new_name: &str) -> String {
    stop_box(box_to_clone);

    get_command_output(
        "distrobox",
        Some(&["create", "--clone", box_to_clone, "--name", new_name]),
    )
}

/// Uninstalls an application from inside a box by running the distro's
/// package manager via `sudo` in a terminal, so the user sees what will be
/// removed and can answer the manager's prompt.
///
/// `app_exec` is the raw `Exec=` value of the application's desktop file.
/// The binary usually is not named after its package (gimp lives in
/// gimp-2.10, for instance), so instead of guessing, the box's own package
/// manager is asked which package owns the binary; only if that fails does
/// the bare executable name serve as the guess. The host-side `.desktop`
/// export is left alone - removing it is a separate, reversible action the
/// user can take from the same row.
pub fn uninstall_app_in_box(box_name: String, image: String, app_exec: String) {
    // Unknown images fall back to apt, as the .deb install path does: it
    // fails loudly in the terminal rather than half-working.
    let manager = detect_pkg_manager(&image).unwrap_or(PkgManager::Apt);
    let (remove_bin, remove_arg) = manager_remove_invocation(manager);

    let package = resolve_package_for_binary(&box_name, manager, &app_exec)
        .unwrap_or_else(|| first_token(&app_exec).to_string());

    let mut args: Vec<&str> = Vec::new();
    if let Some(arg) = remove_arg {
        args.push(arg);
    }
    args.push(&package);
    run_pkg_command_in_terminal(&box_name, remove_bin, &args);
}

/// The first whitespace-separated token of an `Exec=` line - the executable
/// itself, with any arguments dropped.
fn first_token(exec_line: &str) -> &str {
    exec_line.split_whitespace().next().unwrap_or(exec_line)
}

/// How a given manager spells "remove this package": the binary to call and
/// its removal argument, if it takes one. Not always the manager itself -
/// slackware installs with installpkg but removes with removepkg.
fn manager_remove_invocation(manager: PkgManager) -> (&'static str, Option<&'static str>) {
    match manager {
        PkgManager::Apt => ("apt", Some("remove")),
        PkgManager::Dnf => ("dnf", Some("remove")),
        PkgManager::Zypper => ("zypper", Some("remove")),
        PkgManager::Pacman => ("pacman", Some("-R")),
        PkgManager::Apk => ("apk", Some("del")),
        PkgManager::Xbps => ("xbps-remove", None),
        PkgManager::Emerge => ("emerge", Some("--unmerge")),
        PkgManager::Installpkg => ("removepkg", None),
    }
}

/// The package name owning `exec_line`'s binary, according to the box's own
/// package manager. Asks `command -v` inside the box for the full path first,
/// then the manager who owns that path. Returns None for managers we do not
/// know how to ask, or when either step comes back empty - the caller then
/// falls back to the bare executable name.
///
/// The lookup also searches the games directories: desktop files can point
/// there (Debian's cowsay lives in /usr/games) while a non-login shell's PATH
/// does not include them. Both queries pass the untrusted values as positional
/// parameters rather than splicing them into shell text.
fn resolve_package_for_binary(
    box_name: &str,
    manager: PkgManager,
    exec_line: &str,
) -> Option<String> {
    let binary = first_token(exec_line);

    let path_out = get_command_output(
        "distrobox",
        Some(&[
            "enter",
            box_name,
            "--",
            "bash",
            "-c",
            "PATH=\"$PATH:/usr/games:/usr/local/games\" command -v -- \"$1\"",
            "_",
            binary,
        ]),
    );
    let path = path_out
        .lines()
        .find(|l| l.starts_with('/'))?
        .trim()
        .to_string();

    let owner_out = match manager {
        PkgManager::Apt => get_command_output(
            "distrobox",
            Some(&["enter", box_name, "--", "dpkg", "-S", &path]),
        ),
        PkgManager::Dnf | PkgManager::Zypper => get_command_output(
            "distrobox",
            Some(&[
                "enter",
                box_name,
                "--",
                "rpm",
                "-qf",
                "--queryformat",
                "%{NAME}",
                &path,
            ]),
        ),
        PkgManager::Pacman => get_command_output(
            "distrobox",
            Some(&["enter", box_name, "--", "pacman", "-Qqo", &path]),
        ),
        _ => return None,
    };

    parse_package_owner(manager, &owner_out)
}

/// Pulls the package name out of an ownership query's output.
/// dpkg says `cowsay: /usr/games/cowsay` (or `libc6:amd64: /lib/...`),
/// rpm prints the bare name thanks to --queryformat, pacman -Qqo prints the
/// bare name on its own line.
fn parse_package_owner(manager: PkgManager, output: &str) -> Option<String> {
    let line = output.lines().map(str::trim).find(|l| !l.is_empty())?;

    if line.contains("no path found") || line.contains("not owned") || line.contains("error") {
        return None;
    }

    let name = match manager {
        PkgManager::Apt => line.split(':').next()?,
        _ => line,
    };

    let name = name.trim();
    if name.is_empty() {
        return None;
    }

    Some(name.to_string())
}

#[cfg(test)]
mod stream_tests {
    use super::{build_create_args, create_box_streaming, exported_desktop_file_name};

    #[test]
    fn exported_desktop_file_is_named_box_dash_desktop_file() {
        // distrobox-export writes "<container>-<desktop file>" on the host;
        // this is what the Add To Menu / Remove From Menu state is read from.
        assert_eq!(
            exported_desktop_file_name("second", "org.gnome.TextEditor"),
            "second-org.gnome.TextEditor.desktop"
        );
    }

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
        use super::{delete_box, get_all_distroboxes, get_command_output};
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

        // The streamed run now includes the first-enter setup, so by the time it
        // returns the container must be built and usable - a plain enter should
        // run a command and come straight back, without doing setup again.
        let ready = get_command_output("distrobox", Some(&["enter", name, "--", "echo", "READY"]));

        let _ = delete_box(name);
        assert!(exists, "streaming create did not produce a listable box");
        assert!(
            ready.contains("READY"),
            "box was not set up and ready after streaming create, got: {ready}"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{first_token, manager_remove_invocation, parse_package_owner};
    use crate::utils::PkgManager;

    #[test]
    fn first_token_drops_arguments() {
        assert_eq!(first_token("gimp-2.10 --new-instance"), "gimp-2.10");
        assert_eq!(first_token("cowsay"), "cowsay");
        assert_eq!(first_token("  spaced   out  "), "spaced");
    }

    #[test]
    fn parses_dpkg_ownership() {
        assert_eq!(
            parse_package_owner(PkgManager::Apt, "cowsay: /usr/games/cowsay\n"),
            Some("cowsay".to_string())
        );
        // multi-arch packages carry the architecture after a second colon
        assert_eq!(
            parse_package_owner(
                PkgManager::Apt,
                "libc6:amd64: /lib/x86_64-linux-gnu/libc.so.6\n"
            ),
            Some("libc6".to_string())
        );
        assert_eq!(
            parse_package_owner(
                PkgManager::Apt,
                "dpkg-query: no path found matching pattern /x\n"
            ),
            None
        );
    }

    #[test]
    fn parses_rpm_and_pacman_ownership() {
        assert_eq!(
            parse_package_owner(PkgManager::Dnf, "cowsay"),
            Some("cowsay".to_string())
        );
        assert_eq!(
            parse_package_owner(PkgManager::Pacman, "cowsay\n"),
            Some("cowsay".to_string())
        );
        assert_eq!(
            parse_package_owner(PkgManager::Pacman, "error: No package owns /usr/bin/x\n"),
            None
        );
        assert_eq!(parse_package_owner(PkgManager::Dnf, "\n"), None);
    }

    #[test]
    fn removal_is_spelled_per_manager() {
        assert_eq!(
            manager_remove_invocation(PkgManager::Apt),
            ("apt", Some("remove"))
        );
        assert_eq!(
            manager_remove_invocation(PkgManager::Pacman),
            ("pacman", Some("-R"))
        );
        // slackware installs with installpkg but removes with removepkg
        assert_eq!(
            manager_remove_invocation(PkgManager::Installpkg),
            ("removepkg", None)
        );
    }
}

#[cfg(test)]
mod assemble_tests {
    use super::build_assemble_ini;

    #[test]
    fn minimal_ini_has_only_section_and_image() {
        assert_eq!(
            build_assemble_ini(
                "dev",
                "docker.io/library/ubuntu:latest",
                "",
                "",
                false,
                false
            ),
            "[dev]\nimage=docker.io/library/ubuntu:latest\n"
        );
    }

    #[test]
    fn optional_fields_appear_only_when_set() {
        let ini = build_assemble_ini("work", "img", "git, vim", "/home/me/work", true, true);
        assert_eq!(
            ini,
            "[work]\nimage=img\nadditional_packages=\"git, vim\"\nhome=/home/me/work\ninit=true\nnvidia=true\n"
        );
    }

    #[test]
    fn blank_optionals_are_skipped() {
        let ini = build_assemble_ini("d", "i", "   ", "  ", false, false);
        assert!(!ini.contains("additional_packages"));
        assert!(!ini.contains("home="));
        assert!(!ini.contains("init="));
    }
}

#[cfg(test)]
mod ini_preview_tests {
    use super::parse_assemble_ini;

    fn kv(k: &str, v: &str) -> (String, String) {
        (k.to_string(), v.to_string())
    }

    #[test]
    fn parses_a_single_section_with_its_keys_in_order() {
        let ini = "[dev]\nimage=docker.io/library/ubuntu:24.04\nadditional_packages=\"git vim\"\ninit=true\n";
        let s = parse_assemble_ini(ini);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].0, "dev");
        assert_eq!(
            s[0].1,
            vec![
                kv("image", "docker.io/library/ubuntu:24.04"),
                kv("additional_packages", "git vim"),
                kv("init", "true"),
            ]
        );
    }

    #[test]
    fn parses_multiple_sections() {
        let ini = "[a]\nimage=alpine\n\n[b]\nimage=fedora\nnvidia=true\n";
        let s = parse_assemble_ini(ini);
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].0, "a");
        assert_eq!(s[1].0, "b");
        assert_eq!(s[1].1, vec![kv("image", "fedora"), kv("nvidia", "true")]);
    }

    /// The whole point of the preview: keys BoxBuddy has no field for must
    /// still be captured, so the dialog can show them rather than hiding
    /// what the file will actually do.
    #[test]
    fn keeps_unknown_keys_verbatim() {
        let ini = "[x]\nimage=ubuntu\npull=true\ninit_hooks=curl example.com | sh\n";
        let s = parse_assemble_ini(ini);
        assert!(s[0].1.contains(&kv("pull", "true")));
        assert!(s[0].1.contains(&kv("init_hooks", "curl example.com | sh")));
    }

    #[test]
    fn skips_comments_blank_lines_and_junk() {
        let ini = "# a comment\n; another\n[d]\n\nnonsense-without-equals\nimage=debian\n";
        let s = parse_assemble_ini(ini);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].1, vec![kv("image", "debian")]);
    }

    #[test]
    fn keys_before_any_section_are_ignored() {
        let ini = "image=orphan\n[real]\nimage=ubuntu\n";
        let s = parse_assemble_ini(ini);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].0, "real");
    }

    #[test]
    fn empty_input_yields_no_sections() {
        assert!(parse_assemble_ini("").is_empty());
        assert!(parse_assemble_ini("# just a comment\n").is_empty());
    }
}

#[cfg(test)]
mod export_tests {
    use super::desktop_file_path;

    #[test]
    fn builds_the_in_container_desktop_path_from_an_id() {
        assert_eq!(
            desktop_file_path("org.gnome.TextEditor"),
            "/usr/share/applications/org.gnome.TextEditor.desktop"
        );
        // A plain, single-word id is handled the same way.
        assert_eq!(
            desktop_file_path("gimp"),
            "/usr/share/applications/gimp.desktop"
        );
    }

    #[test]
    fn dispatcher_script_round_trips_with_host_and_boxes() {
        use super::{dispatcher_script, parse_dispatcher_marker};

        let cases: Vec<(Option<&str>, Vec<String>)> = vec![
            (Some("/usr/bin/claude"), vec![]),
            (None, vec!["bx1".to_string()]),
            (
                Some("/usr/bin/x"),
                vec!["a".to_string(), "b".to_string(), "c".to_string()],
            ),
            (
                None,
                vec!["a".to_string(), "b".to_string(), "c".to_string()],
            ),
        ];
        for (host, boxes) in cases {
            let script = dispatcher_script("name", host, &boxes);
            let parsed = parse_dispatcher_marker(&script);
            assert!(parsed.is_some(), "parse failed for {:?}", (host, &boxes));
            let (h, b) = parsed.unwrap();
            assert_eq!(h.as_deref(), host, "host mismatch for {:?}", (host, &boxes));
            assert_eq!(b, boxes, "boxes mismatch for {:?}", (host, &boxes));
        }
    }

    #[test]
    fn dispatcher_script_round_trips_no_boxes() {
        use super::{dispatcher_script, parse_dispatcher_marker};

        let script = dispatcher_script("solo", Some("/usr/bin/solo"), &[]);
        let (h, b) = parse_dispatcher_marker(&script).unwrap();
        assert_eq!(h, Some("/usr/bin/solo".to_string()));
        assert!(b.is_empty());
    }

    #[test]
    fn parse_dispatcher_marker_missing_returns_none() {
        use super::parse_dispatcher_marker;
        assert!(parse_dispatcher_marker("not a dispatcher\n").is_none());
        assert!(parse_dispatcher_marker("").is_none());
        assert!(parse_dispatcher_marker("# something else\n").is_none());
    }

    #[test]
    fn parse_dispatcher_marker_empty_host_token_yields_none_host() {
        use super::parse_dispatcher_marker;
        let s = "# boxbuddy-dispatcher: command=x host= boxes=a,b\n";
        let (h, b) = parse_dispatcher_marker(s).unwrap();
        assert!(h.is_none());
        assert_eq!(b, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn parse_dispatcher_marker_preserves_box_order() {
        use super::parse_dispatcher_marker;
        let s = "# boxbuddy-dispatcher: command=x host= boxes=c,a,b\n";
        let (_, b) = parse_dispatcher_marker(s).unwrap();
        assert_eq!(b, vec!["c".to_string(), "a".to_string(), "b".to_string()]);
    }

    #[test]
    fn valid_command_name_accepts_valid_names() {
        use super::valid_command_name;
        assert!(valid_command_name("claude"));
        assert!(valid_command_name("my-tool2"));
        assert!(valid_command_name("a.b+c"));
        assert!(valid_command_name("x"));
        assert!(valid_command_name("a_b"));
    }

    #[test]
    fn valid_command_name_rejects_invalid_names() {
        use super::valid_command_name;
        assert!(!valid_command_name(""));
        assert!(!valid_command_name("has space"));
        assert!(!valid_command_name("../x"));
        assert!(!valid_command_name("-flag"));
        assert!(!valid_command_name("a/b"));
        assert!(!valid_command_name("a'b"));
        assert!(!valid_command_name("a$b"));
        assert!(!valid_command_name(".hidden"));
        assert!(!valid_command_name("-"));
    }

    /// Generates a dispatcher script, writes it to a temp file and asks
    /// `bash -n` to confirm it parses. The dispatcher runs without any
    /// external binaries in the no-op "no targets" branch, but the
    /// richer scripts with host and boxes also have to be clean.
    #[test]
    fn dispatcher_script_is_bash_n_clean() {
        use super::dispatcher_script;
        use std::fs;
        use std::process::Command;

        let variants: Vec<(Option<&str>, Vec<String>)> = vec![
            (Some("/usr/bin/x"), vec!["a".to_string(), "b".to_string()]),
            (None, vec!["a".to_string()]),
            (Some("/usr/bin/x"), vec![]),
            (None, vec![]),
        ];
        for (host, boxes) in variants {
            let script = dispatcher_script("name", host, &boxes);
            let mut path = std::env::temp_dir();
            path.push(format!("boxbuddy_disp_{p}.sh", p = std::process::id()));
            fs::write(&path, &script).unwrap();
            let status = Command::new("bash")
                .args(["-n", path.to_str().unwrap()])
                .status()
                .unwrap();
            assert!(
                status.success(),
                "bash -n failed for host={:?} boxes={:?}",
                host,
                boxes
            );
            let _ = fs::remove_file(&path);
        }
    }

    /// Writes a dispatcher for host=None, boxes=["bx"] plus a fake
    /// `distrobox` executable (a tiny sh script that logs "$@" to a file)
    /// into a fresh temp dir, then runs the dispatcher with PATH
    /// prepended by that temp dir and `BOXBUDDY_DISPATCH=bx`, stdin from
    /// /dev/null. The fake distrobox log should record the exec args.
    #[test]
    fn dispatcher_runs_box_target_via_env() {
        use super::dispatcher_script;
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        use std::process::{Command, Stdio};

        let tmp =
            std::env::temp_dir().join(format!("boxbuddy_dispatch_box_{}", std::process::id()));
        fs::create_dir_all(&tmp).unwrap();

        let log_path = tmp.join("log");
        let fake_dx = tmp.join("distrobox");
        let fake_dx_content = format!(
            "#!/bin/sh\necho \"$@\" >> '{}'\n",
            log_path.to_str().unwrap()
        );
        fs::write(&fake_dx, fake_dx_content).unwrap();
        let mut perm = fs::metadata(&fake_dx).unwrap().permissions();
        perm.set_mode(0o755);
        fs::set_permissions(&fake_dx, perm).unwrap();

        let dispatcher = tmp.join("dispatcher");
        let script = dispatcher_script("mycmd", None, &["bx".to_string()]);
        fs::write(&dispatcher, &script).unwrap();
        let mut perm = fs::metadata(&dispatcher).unwrap().permissions();
        perm.set_mode(0o755);
        fs::set_permissions(&dispatcher, perm).unwrap();

        // Prepend the temp dir so the fake `distrobox` is found first; the
        // system PATH must still be reachable because the dispatcher's
        // `#!/usr/bin/env bash` shebang resolves bash through it.
        let mut path_env = tmp.to_str().unwrap().to_string();
        if let Ok(existing) = std::env::var("PATH") {
            path_env.push(':');
            path_env.push_str(&existing);
        }

        let status = Command::new(&dispatcher)
            .env("PATH", &path_env)
            .env("BOXBUDDY_DISPATCH", "bx")
            .stdin(Stdio::null())
            .status()
            .unwrap();
        assert!(
            status.success(),
            "dispatcher exited with status: {:?}",
            status.code()
        );
        let log_content = fs::read_to_string(&log_path).unwrap();
        assert!(
            log_content.contains("enter bx -- mycmd"),
            "log was: {:?}",
            log_content
        );

        let _ = fs::remove_dir_all(&tmp);
    }

    /// Same shape as `dispatcher_runs_box_target_via_env`, but with a
    /// host target instead of a box. The fake `hostbin` sh script logs
    /// its args; the dispatcher should exec it directly when
    /// `BOXBUDDY_DISPATCH=host`. We pass an extra arg through so the log
    /// has something to assert against (with $@ empty the host binary
    /// would log a blank line and prove nothing).
    #[test]
    fn dispatcher_runs_host_target_via_env() {
        use super::dispatcher_script;
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        use std::process::{Command, Stdio};

        let tmp =
            std::env::temp_dir().join(format!("boxbuddy_dispatch_host_{}", std::process::id()));
        fs::create_dir_all(&tmp).unwrap();

        let log_path = tmp.join("log");
        let host_bin = tmp.join("hostbin");
        let host_bin_content = format!(
            "#!/bin/sh\necho \"$@\" >> '{}'\n",
            log_path.to_str().unwrap()
        );
        fs::write(&host_bin, host_bin_content).unwrap();
        let mut perm = fs::metadata(&host_bin).unwrap().permissions();
        perm.set_mode(0o755);
        fs::set_permissions(&host_bin, perm).unwrap();

        let host_path = host_bin.to_str().unwrap().to_string();
        let dispatcher = tmp.join("dispatcher");
        let script = dispatcher_script("mycmd", Some(&host_path), &[]);
        fs::write(&dispatcher, &script).unwrap();
        let mut perm = fs::metadata(&dispatcher).unwrap().permissions();
        perm.set_mode(0o755);
        fs::set_permissions(&dispatcher, perm).unwrap();

        let status = Command::new(&dispatcher)
            .env("BOXBUDDY_DISPATCH", "host")
            .arg("caller-arg")
            .stdin(Stdio::null())
            .status()
            .unwrap();
        assert!(
            status.success(),
            "dispatcher exited with status: {:?}",
            status.code()
        );
        let log_content = fs::read_to_string(&log_path).unwrap();
        assert!(
            log_content.contains("caller-arg"),
            "log was: {:?}",
            log_content
        );

        let _ = fs::remove_dir_all(&tmp);
    }

    /// The real wrapper text `distrobox-export --bin` writes (as produced
    /// from the template in `/usr/bin/distrobox-export`). The parser should
    /// pull `mybox` out of the `# name:` comment.
    #[test]
    fn parse_distrobox_wrapper_box_reads_real_distrobox_export_wrapper() {
        use super::parse_distrobox_wrapper_box;
        let wrapper = r#"#!/bin/sh
# distrobox_binary
# name: mybox
if [ -z "${CONTAINER_ID}" ]; then
	exec "/usr/bin/distrobox-enter"  -n mybox  -- '/usr/bin/tool' "$@"
elif [ -n "${CONTAINER_ID}" ] && [ "${CONTAINER_ID}" != "mybox" ]; then
	exec distrobox-host-exec '/home/user/.local/bin/tool' "$@"
else
	exec '/usr/bin/tool' "$@"
fi
"#;
        assert_eq!(
            parse_distrobox_wrapper_box(wrapper),
            Some("mybox".to_string())
        );
    }

    /// Older wrappers don't carry the `# name:` comment; the parser should
    /// fall back to the `-n <box>` argument on the distrobox-enter exec
    /// line.
    #[test]
    fn parse_distrobox_wrapper_box_falls_back_to_dash_n_exec_line() {
        use super::parse_distrobox_wrapper_box;
        let wrapper = r#"#!/bin/sh
# distrobox_binary
exec "/usr/bin/distrobox-enter" -n mybox -- '/usr/bin/tool' "$@"
"#;
        assert_eq!(
            parse_distrobox_wrapper_box(wrapper),
            Some("mybox".to_string())
        );
    }

    /// A script with neither marker is not a distrobox wrapper; the parser
    /// must return None so the caller treats it as a plain host binary.
    #[test]
    fn parse_distrobox_wrapper_box_returns_none_for_plain_script() {
        use super::parse_distrobox_wrapper_box;
        let script = "#!/bin/sh\necho hi\n";
        assert_eq!(parse_distrobox_wrapper_box(script), None);
    }
}
