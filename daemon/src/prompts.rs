//! Asking the user, through `org.freedesktop.impl.portal.Access` on the
//! CrownOS portal backend. Each prompt blocks a thread, never the loop.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};

use calloop::channel::Sender;
use crownos_config::schema::Decision;
use crownplugin_model::{Capability, Level};
use zbus::zvariant::{ObjectPath, OwnedValue, Value};

use crate::daemon::Message;

const PORTAL: &str = "org.freedesktop.impl.portal.desktop.crownos";
const PATH: &str = "/org/freedesktop/portal/desktop";
const ACCESS: &str = "org.freedesktop.impl.portal.Access";
/// Read by the CrownOS portal to pick the warning style; other backends ignore it.
pub const LEVEL_OPTION: &str = "crownos-level";

static NEXT: AtomicU32 = AtomicU32::new(1);

fn dialog(app: &str, name: &str, capability: &Capability) -> zbus::Result<u32> {
    let connection = zbus::blocking::Connection::session()?;
    let handle = format!(
        "/org/freedesktop/portal/desktop/request/crownplugind/p{}",
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let dangerous = capability.level() == Level::Dangerous;
    let mut options: HashMap<&str, Value> = HashMap::new();
    options.insert("grant_label", Value::from("Allow"));
    options.insert("deny_label", Value::from("Don't Allow"));
    options.insert(
        LEVEL_OPTION,
        Value::from(if dangerous { "dangerous" } else { "sensitive" }),
    );
    let body = if dangerous {
        "This gives the plugin powerful access to your computer. Only allow it if you trust where it came from."
    } else {
        "You can change this later in Settings → Plugins."
    };
    let reply = connection.call_method(
        Some(PORTAL),
        PATH,
        Some(ACCESS),
        "AccessDialog",
        &(
            ObjectPath::try_from(handle.as_str())?,
            app,
            "",
            format!("Allow “{name}” to {}?", lower_first(&capability.summary())),
            name,
            body,
            options,
        ),
    )?;
    let (response, _results): (u32, HashMap<String, OwnedValue>) = reply.body().deserialize()?;
    Ok(response)
}

fn lower_first(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|first| first.to_lowercase().chain(chars).collect())
        .unwrap_or_default()
}

pub fn ask(sender: Sender<Message>, id: &str, name: &str, capability: &Capability) {
    let (entity, name, capability) = (id.to_owned(), name.to_owned(), capability.clone());
    let spawned = std::thread::Builder::new().name("prompt".into()).spawn(move || {
        let decision = match dialog(&entity, &name, &capability) {
            Ok(0) => Some(Decision::Granted),
            Ok(1) => Some(Decision::Denied),
            Ok(_) => None,
            Err(error) => {
                tracing::warn!("cannot ask about {}: {error}", capability.grant_key());
                None
            }
        };
        let _ = sender.send(Message::Answered {
            entity,
            grant: capability.grant_key().into_owned(),
            decision,
        });
    });
    if let Err(error) = spawned {
        tracing::warn!("cannot start a prompt: {error}");
    }
}
