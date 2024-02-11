use crate::utils::{
    get_command_output, get_host_desktop_files, get_repository_list,
    get_terminal_and_separator_arg, is_flatpak, is_nvidia, run_command,
};
use std::process::Command;

pub struct DBox {
    pub name: String,
    pub distro: String,
    pub image_url: String,
    pub container_id: String,
    pub status: String,
    pub is_running: bool,
}

#[derive(Debug, Clone)]
pub struct DBoxApp {
    pub name: String,
    pub exec_name: String,
    pub icon: String,
    pub desktop_file: String,
    pub is_on_host: bool,
}

pub struct ColsIndexes {
    pub name: usize,
    pub image: usize,
    pub id: usize,
    pub status: usize,
}

pub fn get_all_distroboxes() -> Vec<DBox> {
    let mut my_boxes: Vec<DBox> = vec![];

    let output = get_command_output(String::from("distrobox"), Some(&["list", "--no-color"]));

    let headings = output
        .split('\n')
        .next()
        .unwrap()
        .split('|')
        .map(|h| h.trim())
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

        let box_line = line.split('|').map(|l| l.trim()).collect::<Vec<&str>>();
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

        //println!("line: {:?}", line);
    }

    my_boxes
}

pub fn try_parse_distro_name_from_url(url: &str) -> String {
    let distros = [
        "alma",
        "alpine",
        "amazon",
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
        "ubuntu",
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

pub fn open_terminal_in_box(box_name: String) {
    let (term, sep) = get_terminal_and_separator_arg();

    if is_flatpak() {
        Command::new("flatpak-spawn")
            .arg("--host")
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

pub fn export_app_from_box(app_name: String, box_name: String) -> String {
    get_command_output(
        String::from("distrobox"),
        Some(&[
            "enter",
            &box_name,
            "--",
            "distrobox-export",
            "-a",
            &app_name,
        ]),
    )
}

pub fn remove_app_from_host(app_name: String, box_name: String) -> String {
    get_command_output(
        String::from("distrobox"),
        Some(&[
            "enter",
            &box_name,
            "--",
            "distrobox-export",
            "-a",
            &app_name,
            "-d",
        ]),
    )
}

pub fn run_command_in_box(command: String, box_name: String) {
    if is_flatpak() {
        Command::new(String::from("flatpak-spawn"))
            .args(["--host", "distrobox", "enter", &box_name, "--", &command])
            .spawn()
            .unwrap();
    } else {
        Command::new(String::from("distrobox"))
            .args(["enter", &box_name, "--", &command])
            .spawn()
            .unwrap();
    }
}

pub fn upgrade_box(box_name: String) {
    let (term, sep) = get_terminal_and_separator_arg();
    let command = format!("distrobox upgrade {box_name}; distrobox enter {box_name}");

    if is_flatpak() {
        Command::new("flatpak-spawn")
            .arg("--host")
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

pub fn delete_box(box_name: String) -> String {
    get_command_output(String::from("distrobox"), Some(&["rm", &box_name, "-f"]))
}

pub fn create_box(
    box_name: String,
    image: String,
    home_path: String,
    use_init: bool,
    volumes: Vec<String>,
) -> String {
    let mut args = vec!["create", "-n", &box_name, "-i", &image, "-Y"];
    if is_nvidia() {
        args.push("--nvidia");
    }

    if use_init {
        args.push("--init");
    }

    if !home_path.is_empty() {
        args.push("--home");
        args.push(&home_path);
    }

    if !volumes.is_empty() {
        for vol in volumes.iter() {
            args.push("--volume");
            args.push(vol.as_str());
        }
    }

    get_command_output(String::from("distrobox"), Some(args.as_slice()))
}

pub fn assemble_box(ini_file: String) -> String {
    let args = vec!["assemble", "create", "--file", &ini_file];
    get_command_output(String::from("distrobox"), Some(args.as_slice()))
}

pub fn get_available_images_with_distro_name() -> Vec<String> {
    let existing_images = get_repository_list();
    let output = get_command_output(String::from("distrobox"), Some(&["create", "-C"]));

    let mut imgs: Vec<String> = Vec::new();

    for line in output.split('\n') {
        if line.is_empty() || line == "Images" {
            continue;
        }

        let distro = try_parse_distro_name_from_url(line);
        let mut pretty_line = if distro != "zunknown" {
            format!("{} - {}", distro, line)
        } else {
            format!("unknown - {}", line)
        };

        if existing_images.contains(&line.to_string()) {
            pretty_line = format!("{pretty_line} ✦ ");
        }

        imgs.push(pretty_line);
    }

    imgs.sort();

    imgs
}

pub fn get_apps_in_box(box_name: String) -> Vec<DBoxApp> {
    let mut apps: Vec<DBoxApp> = Vec::new();

    // get list of host apps to check against afterwards
    let host_apps = get_host_desktop_files();

    let desktop_files = get_command_output(
        String::from("distrobox"),
        Some(&[
            "enter",
            &box_name,
            "--",
            "bash",
            "-c",
            "grep -L \"NoDisplay=true\" /usr/share/applications/*.desktop",
        ]),
    );

    for line in desktop_files.split('\n') {
        if line.is_empty() || line.contains("No such file") || !line.starts_with("/usr/share") {
            continue;
        }

        // I've no idea why I can pipe here, but if I try and pipe above I just get weird errors
        let get_pieces_cmd = get_command_output(
            String::from("distrobox"),
            Some(&[
                "enter",
                &box_name,
                "--",
                "bash",
                "-c",
                &format!(
                    "NAME=$(grep -m 1 \"^Name=\" {} 
            | sed 's/^Name=//'); 
            EXEC=$(grep -m 1 \"^Exec=\" {} 
            | sed 's/^Exec=//' | tr -d '\n'); 
            ICON=$(grep -m 1 \"^Icon=\" {} 
            | sed 's/^Icon=//' | tr -d '\n'); 
            echo \"${{NAME}} | ${{EXEC}} | ${{ICON}}\"",
                    line, line, line
                ),
            ]),
        );

        let pieces = get_pieces_cmd
            .split('|')
            .map(|l| l.trim())
            .collect::<Vec<&str>>();

        if pieces.len() < 3 || pieces[0].is_empty() {
            continue;
        }

        // figure out if this exists on the host so we can show remove btn instead
        let desktop_file_name = line
            .replace("/usr/share/applications/", "")
            .replace(".desktop", "");

        let host_desktop_name = format!("{box_name}-{desktop_file_name}.desktop");

        let app = DBoxApp {
            name: String::from(pieces[0]),
            exec_name: pieces[1].replace("%F", "").replace("%U", ""),
            icon: String::from(pieces[2]),
            desktop_file: desktop_file_name,
            is_on_host: host_apps.contains(&host_desktop_name),
        };

        apps.push(app);
    }

    apps
}

pub fn stop_box(box_name: String) {
    let _ = run_command(String::from("distrobox"), Some(&["stop", &box_name, "-Y"]));
}

pub fn get_number_of_boxes() -> u32 {
    let output = get_command_output(String::from("distrobox"), Some(&["list", "--no-color"]));

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

pub fn install_deb_in_box(box_name: String, file_path: String) {
    let (term, sep) = get_terminal_and_separator_arg();

    if is_flatpak() {
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

pub fn install_rpm_in_box(box_name: String, file_path: String) {
    let (term, sep) = get_terminal_and_separator_arg();

    //TODO this needs to be done when fetching boxes at the beginning
    let mut pkg_man = String::from("dnf");
    let my_boxes = get_all_distroboxes();
    for dbox in my_boxes {
        if dbox.name == box_name && dbox.distro == "opensuse" {
            pkg_man = String::from("zypper");
        }
    }

    if is_flatpak() {
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
