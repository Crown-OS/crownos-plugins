//! Slow work off the loop: HTTP, programs, notifications and files. Results
//! come back as [`Message::Completed`].

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use calloop::channel::Sender;
use crownplugin_model::UserDir;
use crownplugin_model::package::relative_path;

use crate::daemon::{Completion, Message};
use crate::vm::Command as ScriptCommand;

const HTTP_TIMEOUT: Duration = Duration::from_secs(15);
const EXEC_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_OUTPUT: u64 = 1024 * 1024;
const MAX_FILE: u64 = 4 * 1024 * 1024;

pub fn complete(sender: &Sender<Message>, entity: &str, request: u32, completion: Completion) {
    let _ = sender.send(Message::Completed {
        entity: entity.to_owned(),
        request,
        completion,
    });
}

/// Whether `url` is http(s) to one of `hosts` or a subdomain of one.
pub fn host_allowed(url: &str, hosts: &[String]) -> bool {
    let Some(rest) = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://")) else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = authority.rsplit('@').next().unwrap_or_default();
    let host = host.split(':').next().unwrap_or_default().to_ascii_lowercase();
    hosts.iter().any(|allowed| {
        let allowed = allowed.to_ascii_lowercase();
        host == allowed || host.ends_with(&format!(".{allowed}"))
    })
}

fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::Agent::config_builder()
            .timeout_global(Some(HTTP_TIMEOUT))
            .http_status_as_error(false)
            .user_agent("crownplugind")
            .build()
            .into()
    })
}

fn spawn(name: &str, work: impl FnOnce() + Send + 'static) {
    if let Err(error) = std::thread::Builder::new().name(name.into()).spawn(work) {
        tracing::warn!("cannot start a {name} worker: {error}");
    }
}

pub fn http(sender: Sender<Message>, entity: &str, request: u32, post: bool, url: String, body: Option<String>) {
    let entity = entity.to_owned();
    spawn("http", move || {
        let response = if post {
            agent().post(&url).send(body.unwrap_or_default())
        } else {
            agent().get(&url).call()
        };
        let completion = match response {
            Ok(mut response) => {
                let status = response.status().as_u16();
                match response.body_mut().with_config().limit(MAX_FILE).read_to_string() {
                    Ok(body) => Completion::Http { status, body },
                    Err(error) => Completion::Error(error.to_string()),
                }
            }
            Err(error) => Completion::Error(error.to_string()),
        };
        complete(&sender, &entity, request, completion);
    });
}

fn read_capped(mut reader: impl Read) -> String {
    let mut bytes = Vec::new();
    let _ = reader.by_ref().take(MAX_OUTPUT).read_to_end(&mut bytes);
    String::from_utf8_lossy(&bytes).into_owned()
}

pub fn exec(sender: Sender<Message>, entity: &str, request: u32, argv: Vec<String>) {
    let entity = entity.to_owned();
    spawn("exec", move || {
        let child = Command::new(&argv[0])
            .args(&argv[1..])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let completion = match child {
            Ok(mut child) => {
                let stdout = child.stdout.take().map(|out| std::thread::spawn(move || read_capped(out)));
                let stderr = child.stderr.take().map(|err| std::thread::spawn(move || read_capped(err)));
                let started = Instant::now();
                let status = loop {
                    match child.try_wait() {
                        Ok(Some(status)) => break Some(status),
                        Ok(None) if started.elapsed() < EXEC_TIMEOUT => {
                            std::thread::sleep(Duration::from_millis(20));
                        }
                        _ => {
                            let _ = child.kill();
                            let _ = child.wait();
                            break None;
                        }
                    }
                };
                let join = |handle: Option<std::thread::JoinHandle<String>>| {
                    handle.and_then(|handle| handle.join().ok()).unwrap_or_default()
                };
                match status {
                    Some(status) => Completion::Exec {
                        code: status.code().unwrap_or(-1),
                        stdout: join(stdout),
                        stderr: join(stderr),
                    },
                    None => Completion::Error(format!("{} ran longer than {EXEC_TIMEOUT:?}", argv[0])),
                }
            }
            Err(error) => Completion::Error(format!("{}: {error}", argv[0])),
        };
        complete(&sender, &entity, request, completion);
    });
}

fn base(dir: UserDir) -> Option<PathBuf> {
    match dir {
        UserDir::Home => dirs::home_dir(),
        UserDir::Documents => dirs::document_dir(),
        UserDir::Downloads => dirs::download_dir(),
        UserDir::Music => dirs::audio_dir(),
        UserDir::Pictures => dirs::picture_dir(),
        UserDir::Videos => dirs::video_dir(),
    }
}

/// `path` inside the user folder `dir`, refusing anything that resolves outside it.
fn resolve(dir: UserDir, path: &str, must_exist: bool) -> Result<PathBuf, String> {
    let base = base(dir).ok_or("that folder does not exist")?;
    let rel = relative_path(path).ok_or("the path leaves the folder")?;
    let full = base.join(rel);
    let base = base.canonicalize().map_err(|error| error.to_string())?;
    let check = if must_exist {
        full.canonicalize()
    } else {
        full.parent()
            .unwrap_or(&base)
            .canonicalize()
            .map(|parent| parent.join(full.file_name().unwrap_or_default()))
    }
    .map_err(|error| error.to_string())?;
    if check.starts_with(&base) {
        Ok(check)
    } else {
        Err("the path leaves the folder".into())
    }
}

fn read_file(path: &Path) -> Result<String, String> {
    let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let mut text = String::new();
    file.take(MAX_FILE)
        .read_to_string(&mut text)
        .map_err(|error| error.to_string())?;
    Ok(text)
}

pub fn files(sender: &Sender<Message>, entity: &str, command: ScriptCommand) {
    let (request, result) = match command {
        ScriptCommand::ReadFile { request, dir, path } => (
            request,
            resolve(dir, &path, true).and_then(|path| read_file(&path)).map(Completion::Text),
        ),
        ScriptCommand::ListDir { request, dir, path } => (
            request,
            resolve(dir, &path, true).and_then(|path| {
                let entries = std::fs::read_dir(path).map_err(|error| error.to_string())?;
                Ok(Completion::Entries(
                    entries
                        .flatten()
                        .map(|entry| entry.file_name().to_string_lossy().into_owned())
                        .collect(),
                ))
            }),
        ),
        ScriptCommand::WriteFile { request, dir, path, text } => (
            request,
            resolve(dir, &path, false).and_then(|path| {
                crate::util::fsx::write_atomic(&path, text.as_bytes())
                    .map(|()| Completion::Done)
                    .map_err(|error| error.to_string())
            }),
        ),
        _ => return,
    };
    complete(sender, entity, request, result.unwrap_or_else(Completion::Error));
}

crownos_ipc::protocol! {
    mod crownotify: "crownotify" {
        fn send_general_notification(
            app_name: String,
            summary: String,
            body: String,
            expire_timeout: u32,
            actions: Vec<String>,
        ) -> u32;
    }
}

pub fn notify(app: String, summary: String, body: String) {
    spawn("notify", move || {
        let sent = crownotify::Client::connect().and_then(|mut client| {
            let pending = client.send_general_notification(app, summary, body, 5000, Vec::new())?;
            client.wait(pending, Some(Duration::from_secs(2)))
        });
        if let Err(error) = sent {
            tracing::warn!("could not post a notification: {error}");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_listed_hosts_and_their_subdomains_are_allowed() {
        let hosts = vec!["example.com".to_owned()];
        assert!(host_allowed("https://example.com/a", &hosts));
        assert!(host_allowed("https://api.example.com:8443/x?y", &hosts));
        assert!(!host_allowed("https://example.com.evil.org/", &hosts));
        assert!(!host_allowed("https://evil.org/?u=example.com", &hosts));
        assert!(!host_allowed("https://example.com@evil.org/", &hosts));
        assert!(!host_allowed("file:///etc/passwd", &hosts));
    }
}
