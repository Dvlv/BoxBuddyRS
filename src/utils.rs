use std::path::Path;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Error, ErrorKind};
use std::process::{Command, Stdio};

pub fn run_command_and_stream_out(
    cmd_to_run: std::string::String,
    args_for_cmd: &[&str],
) -> Result<(), Error> {
    let stdout = Command::new(cmd_to_run)
        .args(args_for_cmd)
        .stdout(Stdio::piped())
        .spawn()?
        .stdout
        .ok_or_else(|| Error::new(ErrorKind::Other, "Could not capture standard output."))?;

    let reader = BufReader::new(stdout);

    reader
        .lines()
        .map_while(Result::ok)
        .filter(|line| line.contains("usb"))
        .for_each(|line| println!("{}", line));

    Ok(())
}

pub fn run_command_and_stream_err(
    cmd_to_run: std::string::String,
    args_for_cmd: &[&str],
) -> Result<(), Error> {
    let stdout = Command::new(cmd_to_run)
        .args(args_for_cmd)
        .env("GIT_EXTERNAL_DIFF", "difft")
        .stderr(Stdio::piped())
        .spawn()?
        .stderr
        .ok_or_else(|| Error::new(ErrorKind::Other, "Could not capture standard output."))?;

    let reader = BufReader::new(stdout);

    reader
        .lines()
        .map_while(Result::ok)
        .filter(|line| line.contains("usb"))
        .for_each(|line| println!("{}", line));

    Ok(())
}

pub fn run_command(
    cmd_to_run: std::string::String,
    args_for_cmd: Option<&[&str]>,
) -> std::result::Result<std::process::Output, std::io::Error> {
    let mut cmd = Command::new(cmd_to_run);
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
    let distro_colours : HashMap<&str, &str> = HashMap::from([

       ( "ubuntu", "#FF4400"),
       ( "debian", "#da5555"),
       ( "centos", "#ff6600"),
       ( "oracle", "#ff0000"),
       ( "fedora", "blue"),
       ( "arch", "#12aaff"),
       ( "alma", "#dadada"),
       ( "slackware", "#6145a7"),
       ( "gentoo", "#daaada"),
       ( "kali", "#000000"),
       ( "alpine", "#2147ea"),
       ( "clearlinux", "#56bbff"),
       ( "void", "#abff12"),
       ( "amazon", "#de5412"),
       ( "rocky", "#91ff91"),
       ( "redhat", "#ff6662"),
       ( "opensuse", "#daff00"),
       ( "mageia", "#b612b6"),
    ]);

    if distro_colours.contains_key(distro) {
        return format!("<span foreground=\"{}\">⬤</span>", distro_colours[distro]);
    }

    return format!("<span foreground=\"{}\">⬤</span>", "#000000");
}

pub fn has_distrobox_installed() -> bool {
    let output = get_command_output(String::from("which"), Some(&["distrobox"]));

    if output.contains("no distrobox in") || output.is_empty() {
        return false
    }

    true

}

pub fn get_terminal_and_separator_arg() -> (String, String) {
    let mut terminal = "gnome-terminal";
    let mut separator_arg = "--";

    let output = get_command_output(String::from("which"), Some(&["gnome-terminal"]));

    if output.contains("no gnome-terminal in") || output.is_empty() {
        terminal = "konsole";
        separator_arg = "-e";
    }

    return (terminal.to_string(), separator_arg.to_string())
}

pub fn is_flatpak() -> bool {
    let fp_env = std::env::var("FLATPAK_ID").is_ok();
    if fp_env {
        return true;
    }

    Path::new("/.flatpak-info").exists()
}