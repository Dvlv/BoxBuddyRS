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
