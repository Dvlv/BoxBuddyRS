use crate::utils::{
    get_command_output, get_host_desktop_files, get_repository_list,
    get_terminal_and_separator_arg, is_flatpak, is_nvidia, run_command,
};
use std::process::Command;

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
        if box_line.len() > 3 {
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
    if term == "blackbox-terminal" {
        // Black Box unlike other terminals gets commands in one argument
        if is_flatpak() {
            if term_is_flatpak {
                Command::new("flatpak-spawn")
                    .arg("--host")
                    .arg("flatpak")
                    .arg("run")
                    .arg(term)
                    .arg(sep)
                    .arg(format!("distrobox enter {box_name}"))
                    .spawn()
                    .unwrap();
            } else {
                Command::new("flatpak-spawn")
                    .arg("--host")
                    .arg(term)
                    .arg(sep)
                    .arg(format!("distrobox enter {box_name}"))
                    .spawn()
                    .unwrap();
            }
        } else {
            if term_is_flatpak {
                Command::new("flatpak")
                    .arg("run")
                    .arg(term)
                    .arg(sep)
                    .arg(format!("distrobox enter {box_name}"))
                    .spawn()
                    .unwrap();
            } else {
                Command::new(term)
                    .arg(sep)
                    .arg(format!("distrobox enter {box_name}"))
                    .spawn()
                    .unwrap();
            }
        }
    } else {
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
pub fn create_box(
    box_name: &str,
    image: &str,
    home_path: &str,
    use_init: bool,
    volumes: &[String],
) -> String {
    let mut args = vec!["create", "-n", box_name, "-i", image, "-Y"];
    if is_nvidia() {
        args.push("--nvidia");
    }

    if use_init {
        args.push("--init");
    }

    if !home_path.is_empty() {
        args.push("--home");
        args.push(home_path);
    }

    if !volumes.is_empty() {
        for vol in volumes {
            args.push("--volume");
            args.push(vol);
        }
    }

    get_command_output("distrobox", Some(args.as_slice()))
}

/// Runs `distrobox-assemble` with the provided file.
pub fn assemble_box(ini_file: &str) -> String {
    let args = &["assemble", "create", "--file", ini_file];
    get_command_output("distrobox", Some(args))
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
    let output = get_command_output("distrobox", Some(&["list", "--no-color"]));

    // I would like to just do output.lines().count() but I get inconsistent results
    let mut count = 0;
    for line in output.lines() {
        if line.starts_with("ID") || line.is_empty() {
            continue;
        }

        count += 1;
    }

    count
}

/// Tries to install a .deb file in the box using `apt`. Spawns a terminal for
/// the user to confirm / cancel.
pub fn install_deb_in_box(box_name: String, file_path: String) {
    let (term, sep, term_is_flatpak) = get_terminal_and_separator_arg();

    if term == "blackbox-terminal" {
        // Black Box unlike other terminals gets commands in one argument
        if is_flatpak() {
            if term_is_flatpak {
                Command::new("flatpak-spawn")
                    .arg("--host")
                    .arg("flatpak")
                    .arg("run")
                    .arg(term)
                    .arg(sep)
                    .arg(format!(
                        "distrobox enter {box_name} -- sudo apt install {file_path}"
                    ))
                    .spawn()
                    .unwrap();
            } else {
                Command::new("flatpak-spawn")
                    .arg("--host")
                    .arg(term)
                    .arg(sep)
                    .arg(format!(
                        "distrobox enter {box_name} -- sudo apt install {file_path}"
                    ))
                    .spawn()
                    .unwrap();
            }
        } else {
            if term_is_flatpak {
                Command::new("flatpak")
                    .arg("run")
                    .arg(term)
                    .arg(sep)
                    .arg(format!(
                        "distrobox enter {box_name} -- sudo apt install {file_path}"
                    ))
                    .spawn()
                    .unwrap();
            } else {
                Command::new(term)
                    .arg(sep)
                    .arg(format!(
                        "distrobox enter {box_name} -- sudo apt install {file_path}"
                    ))
                    .spawn()
                    .unwrap();
            }
        }
    } else {
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
                    .arg("apt")
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
                    .arg("apt")
                    .arg("install")
                    .arg(file_path)
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
                    .arg("--")
                    .arg("sudo")
                    .arg("apt")
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
                    .arg("apt")
                    .arg("install")
                    .arg(file_path)
                    .spawn()
                    .unwrap();
            }
        }
    }
}

/// Tries to install a .rpm file in the box using `zypper` or `dnf`.
/// Spawns a terminal for the user to confirm / cancel.
pub fn install_rpm_in_box(box_name: String, file_path: String) {
    let (term, sep, term_is_flatpak) = get_terminal_and_separator_arg();

    //TODO this needs to be done when fetching boxes at the beginning
    let mut pkg_man = String::from("dnf");
    let my_boxes = get_all_distroboxes();
    for dbox in my_boxes {
        if dbox.name == box_name && dbox.distro == "opensuse" {
            pkg_man = String::from("zypper");
        }
    }

    if term == "blackbox-terminal" {
        // Black Box unlike other terminals gets commands in one argument
        if is_flatpak() {
            if term_is_flatpak {
                Command::new("flatpak-spawn")
                    .arg("--host")
                    .arg("flatpak")
                    .arg("run")
                    .arg(term)
                    .arg(sep)
                    .arg(format!(
                        "distrobox enter {box_name} -- sudo {pkg_man} install {file_path}"
                    ))
                    .spawn()
                    .unwrap();
            } else {
                Command::new("flatpak-spawn")
                    .arg("--host")
                    .arg(term)
                    .arg(sep)
                    .arg(format!(
                        "distrobox enter {box_name} -- sudo {pkg_man} install {file_path}"
                    ))
                    .spawn()
                    .unwrap();
            }
        } else {
            if term_is_flatpak {
                Command::new("flatpak")
                    .arg("run")
                    .arg(term)
                    .arg(sep)
                    .arg(format!(
                        "distrobox enter {box_name} -- sudo {pkg_man} install {file_path}"
                    ))
                    .spawn()
                    .unwrap();
            } else {
                Command::new(term)
                    .arg(sep)
                    .arg(format!(
                        "distrobox enter {box_name} -- sudo {pkg_man} install {file_path}"
                    ))
                    .spawn()
                    .unwrap();
            }
        }
    } else {
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
                    .arg(pkg_man)
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
                    .arg(pkg_man)
                    .arg("install")
                    .arg(file_path)
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
                    .arg("--")
                    .arg("sudo")
                    .arg(pkg_man)
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
                    .arg(pkg_man)
                    .arg("install")
                    .arg(file_path)
                    .spawn()
                    .unwrap();
            }
        }
    }
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
