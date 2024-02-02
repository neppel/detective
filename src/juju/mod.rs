use regex::Regex;
use serde_json::Value;
use std::process::Stdio;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::select;
use tokio::time::{sleep, Duration};
use tokio_util::sync::CancellationToken;

async fn juju_status() -> Value {
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

    let new_status = status.wait_with_output().await.unwrap();

    let data = std::str::from_utf8(&new_status.stdout).unwrap();

    let value: Value = serde_json::from_str(data).unwrap();

    value
}

async fn list_applications() -> Vec<String> {
    let status = juju_status().await;

    let mut applications = Vec::new();

    for (application_name, _) in status["applications"].as_object().unwrap() {
        applications.push(application_name.to_owned());
    }

    applications
}

async fn list_units(application: &str) -> Vec<String> {
    let value = juju_status().await;

    let mut units = Vec::new();

    for (application_name, application_data) in value["applications"].as_object().unwrap() {
        if application_name == application {
            for (unit_name, _) in application_data["units"].as_object().unwrap() {
                units.push(unit_name.to_owned());
            }
        }
    }

    units
}

pub(crate) async fn pause(application: String, token: CancellationToken) {
    let available_applications = list_applications().await;
    if !available_applications.contains(&application) {
        println!("unknown application: {}", application);
        std::process::exit(1);
    }
    let units = list_units(&application).await;
    let mut pause_hooks = Vec::new();
    for unit in units.clone() {
        let pty = pty_process::Pty::new().unwrap();
        pty.resize(pty_process::Size::new(24, 80)).unwrap();
        pause_hooks.push(
            pty_process::Command::new("juju")
                .args(["debug-hooks", unit.as_str()])
                .spawn(&pty.pts().unwrap())
                .unwrap(),
        );
        println!("paused unit: {}", unit.as_str());
        sleep(Duration::from_secs(1)).await;
    }
    println!("press ctrl+c to resume units");
    select! {
        _ = token.cancelled() => {
            println!("resume units");
        }
    }
    for mut hook in pause_hooks {
        hook.kill().await.unwrap();
    }
    for unit in units {
        Command::new("juju")
            .args([
                "ssh",
                unit.as_str(),
                "tmux",
                "kill-session",
                "-t",
                unit.as_str(),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .spawn()
            .unwrap()
            .wait_with_output()
            .await
            .unwrap();
        Command::new("juju")
            .args(["resolve", unit.as_str()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .spawn()
            .unwrap()
            .wait_with_output()
            .await
            .unwrap();
    }
}

pub(crate) async fn trace(application: String, token: CancellationToken) {
    let available_applications = list_applications().await;
    if !available_applications.contains(&application) {
        println!("unknown application: {}", application);
        std::process::exit(1);
    }
    let units = list_units(&application).await;
    match units.len() {
        0 => {
            println!("no units found for application: {}", application);
            std::process::exit(1);
        }
        1 => {
            let unit = units.first().unwrap();
            let loaded_trace_function = include_str!("trace_function.py");
            let number_of_lines = loaded_trace_function.lines().count() + 1;
            let trace_function = loaded_trace_function
                .replace("    ", "\\t")
                .replace('\"', "\\\"")
                .replace("9999", number_of_lines.to_string().as_str());
            let mut dispatch_script: String = "(echo $'".to_owned();
            dispatch_script.push_str(trace_function.as_str());
            dispatch_script.push_str(r#"';sed 's/    ops.main(/    sys.settrace(trace_function)\n    ops.main(/' ./src/charm.py;sed 's/    main(/    sys.settrace(trace_function)\n    main(/' ./src/charm.py;) | JUJU_DISPATCH_PATH='hooks/hook_name' PYTHONPATH=lib:venv /usr/bin/env python3 -"#);
            trace_unit(unit, dispatch_script, token.clone()).await;
        }
        _ => {
            println!("multiple units found for application: {}", application);
            std::process::exit(1);
        }
    }
}

async fn trace_unit(unit_name: &String, dispatch_script: String, token: CancellationToken) {
    let mut pty = pty_process::Pty::new().unwrap();
    pty.resize(pty_process::Size::new(24, 80)).unwrap();
    let mut debug_hooks = pty_process::Command::new("juju")
        .args(["debug-hooks", unit_name])
        .spawn(&pty.pts().unwrap())
        .unwrap();

    let mut child = Command::new("juju")
        .args(["ssh", unit_name, "bash"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .unwrap();

    sleep(Duration::from_secs(5)).await;

    let child_stdin = child.stdin.as_mut().unwrap();
    loop {
        let mut content = "".to_owned();
        loop {
            if token.is_cancelled() {
                println!("cancelling tasks for {}", unit_name);
                child_stdin.write_all(b"exit\n").await.unwrap();
                child.kill().await.unwrap();
                debug_hooks.kill().await.unwrap();
                Command::new("juju")
                    .args(["ssh", unit_name, "tmux", "kill-session", "-t", unit_name])
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .spawn()
                    .unwrap()
                    .wait_with_output()
                    .await
                    .unwrap();
                Command::new("juju")
                    .args(["resolve", unit_name])
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .spawn()
                    .unwrap()
                    .wait_with_output()
                    .await
                    .unwrap();
                return;
            }
            let command = format!("tmux send-keys -t {} \"env\" ENTER\n", unit_name);
            let writing = child_stdin.write(command.as_bytes()).await;
            match writing {
                Ok(_) => (),
                Err(error) => {
                    println!("error: {:?}", error);
                    break;
                }
            }

            let mut buffer = Vec::new();
            match pty.read_buf(&mut buffer).await {
                Ok(_) => {}
                Err(_) => {
                    continue;
                }
            }
            content.push_str(std::str::from_utf8(&buffer).unwrap());

            if content.contains("JUJU_DISPATCH_PATH") {
                let re = Regex::new(r"JUJU_DISPATCH_PATH=hooks/[a-z]*-*[a-z]*\u{001b}").unwrap();
                let captures = re.captures(&content);
                if let Some(captures) = captures {
                    let hook_name = captures
                        .iter()
                        .next()
                        .expect("no captures found")
                        .expect("no captures found")
                        .as_str()
                        .split("JUJU_DISPATCH_PATH=")
                        .nth(1)
                        .unwrap()
                        .split('/')
                        .nth(1)
                        .unwrap()
                        .split('\u{001b}')
                        .next()
                        .unwrap();
                    let patched_dispatch_script = dispatch_script.replace("hook_name", hook_name);
                    let command = format!(
                        "tmux send-keys -t {} \"{}; exit\" ENTER\n",
                        unit_name, patched_dispatch_script
                    );
                    child_stdin.write_all(command.as_bytes()).await.unwrap();
                    println!("{}: dispatching {} hook", unit_name, hook_name);
                    break;
                }
            }
            sleep(Duration::from_millis(100)).await;
        }
        sleep(Duration::from_secs(5)).await;
    }
}
