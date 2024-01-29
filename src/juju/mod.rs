use serde_json::Value;
use std::process::Command;
use std::process::Stdio;

pub(crate) fn list_applications() -> Vec<String> {
    match Command::new("juju").args(["version"]).spawn() {
        Ok(_) => (),
        Err(e) => {
            if let std::io::ErrorKind::NotFound = e.kind() {
                println!("`juju` was not found in your PATH");
                std::process::exit(1);
            }
            println!("some strange error occurred");
            std::process::exit(1);
        }
    }

    let status = Command::new("juju")
        .args(["status", "--format=json"])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let new_status = status.wait_with_output().unwrap();

    let data = std::str::from_utf8(&new_status.stdout).unwrap();

    let value: Value = serde_json::from_str(data).unwrap();

    let mut applications = Vec::new();

    for (application_name, _) in value["applications"].as_object().unwrap() {
        applications.push(application_name.to_owned());
    }

    applications
}
