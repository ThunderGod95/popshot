use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

use crate::output_selection::CaptureMode;
use cosmic::iced::{Subscription, futures::StreamExt};
use tokio::sync::mpsc;
use zbus::{Connection, fdo, zvariant::OwnedValue};

pub const APP_ID: &str = "io.github.tg.PopShot";
const PATH: &str = "/io/github/tg/PopShot";
static REQUESTS: OnceLock<Mutex<Option<mpsc::Receiver<Request>>>> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct Request {
    pub capture_id: Option<String>,
    pub mode: Option<CaptureMode>,
    pub token: Option<String>,
}

struct Application(mpsc::Sender<Request>);

#[zbus::interface(name = "org.freedesktop.Application")]
impl Application {
    async fn activate(&self, platform_data: HashMap<String, OwnedValue>) -> fdo::Result<()> {
        self.send(None, None, platform_data).await
    }

    async fn activate_action(
        &self,
        action_name: &str,
        parameter: Vec<OwnedValue>,
        platform_data: HashMap<String, OwnedValue>,
    ) -> fdo::Result<()> {
        let mode = match action_name {
            "area" => Some(CaptureMode::Rectangle),
            "freehand" => Some(CaptureMode::Freehand),
            "fullscreen" => Some(CaptureMode::Fullscreen),
            _ => None,
        };

        if let Some(mode) = mode {
            if !parameter.is_empty() {
                return Err(fdo::Error::InvalidArgs(
                    "Capture actions take no parameters".into(),
                ));
            }
            return self.send(None, Some(mode), platform_data).await;
        }

        if action_name != "preview" || parameter.len() != 1 {
            return Err(fdo::Error::InvalidArgs(
                "Expected preview with one capture ID".into(),
            ));
        }

        let id = <&str>::try_from(&parameter[0])
            .map_err(|_| fdo::Error::InvalidArgs("Capture ID must be a string".into()))?;

        if !crate::cache::valid_id(id) {
            return Err(fdo::Error::InvalidArgs("Invalid capture ID".into()));
        }

        self.send(Some(id.into()), None, platform_data).await
    }
}

impl Application {
    async fn send(
        &self,
        capture_id: Option<String>,
        mode: Option<CaptureMode>,
        data: HashMap<String, OwnedValue>,
    ) -> fdo::Result<()> {
        let token = data
            .get("activation-token")
            .and_then(|value| <&str>::try_from(value).ok())
            .map(str::to_owned);

        self.0
            .send(Request {
                capture_id,
                mode,
                token,
            })
            .await
            .map_err(|_| fdo::Error::Failed("Application is closing".into()))
    }
}

/// Own the standard application name before starting the UI; never auto-start ourselves.
pub async fn start(service: bool, mode: Option<CaptureMode>) -> zbus::Result<Option<Connection>> {
    let (sender, receiver) = mpsc::channel(32);

    let connection = Connection::session().await?;
    connection
        .object_server()
        .at(PATH, Application(sender.clone()))
        .await?;

    let reply = connection
        .request_name_with_flags(APP_ID, fdo::RequestNameFlags::DoNotQueue.into())
        .await;

    if matches!(reply, Err(zbus::Error::NameTaken)) {
        if !service {
            let proxy =
                zbus::Proxy::new(&connection, APP_ID, PATH, "org.freedesktop.Application").await?;

            let mut data: HashMap<&str, zbus::zvariant::Value<'_>> = HashMap::new();

            if let Ok(token) = std::env::var("XDG_ACTIVATION_TOKEN") {
                data.insert("activation-token", token.into());
            }

            if let Some(mode) = mode {
                let action = match mode {
                    CaptureMode::Rectangle => "area",
                    CaptureMode::Freehand => "freehand",
                    CaptureMode::Fullscreen => "fullscreen",
                };

                let parameters: Vec<OwnedValue> = Vec::new();

                proxy
                    .call::<_, _, ()>("ActivateAction", &(action, parameters, data))
                    .await?;
            } else {
                proxy.call::<_, _, ()>("Activate", &(data,)).await?;
            }
        }

        return Ok(None);
    }

    reply?;

    REQUESTS
        .set(Mutex::new(Some(receiver)))
        .expect("activation initialized once");

    if !service {
        sender
            .send(Request {
                capture_id: None,
                mode,
                token: std::env::var("XDG_ACTIVATION_TOKEN").ok(),
            })
            .await
            .ok();
    }

    Ok(Some(connection))
}

/// Older portals infer native app identity from its systemd application scope.
/// This must run before screenshot/notification calls cache the sender's identity.
pub async fn register_portals(connection: &Connection) -> Result<(), Box<dyn std::error::Error>> {
    if ashpd::register_host_app(APP_ID.parse().expect("valid app ID"))
        .await
        .is_ok()
    {
        return Ok(());
    }

    enter_application_scope(connection).await
}

async fn enter_application_scope(
    connection: &Connection,
) -> Result<(), Box<dyn std::error::Error>> {
    use zbus::zvariant::{OwnedObjectPath, Value};

    let manager = zbus::Proxy::new(
        connection,
        "org.freedesktop.systemd1",
        "/org/freedesktop/systemd1",
        "org.freedesktop.systemd1.Manager",
    )
    .await?;

    let mut jobs = manager.receive_signal("JobRemoved").await?;

    manager.call::<_, _, ()>("Subscribe", &()).await?;

    let result = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        // The suffix must be alphanumeric for older portals' unit-name parser.
        let unit = format!("app-{APP_ID}-{}.scope", uuid::Uuid::new_v4().simple());

        let properties = vec![
            ("PIDs", Value::from(vec![std::process::id()])),
            ("CollectMode", Value::from("inactive-or-failed")),
        ];

        let auxiliary: Vec<(&str, Vec<(&str, Value<'_>)>)> = Vec::new();

        let job: OwnedObjectPath = manager
            .call("StartTransientUnit", &(unit, "fail", properties, auxiliary))
            .await?;

        while let Some(signal) = jobs.next().await {
            let (_, path, _, result): (u32, OwnedObjectPath, String, String) =
                signal.body().deserialize()?;

            if path == job {
                return if result == "done" {
                    Ok(())
                } else {
                    Err(format!("Could not establish PopShot's portal identity: {result}").into())
                };
            }
        }

        Err::<(), Box<dyn std::error::Error>>(
            "Systemd disconnected while establishing portal identity".into(),
        )
    })
    .await;

    let _ = manager.call::<_, _, ()>("Unsubscribe", &()).await;

    result?
}

pub fn subscription() -> Subscription<crate::app::Message> {
    Subscription::run(|| {
        let receiver = REQUESTS
            .get()
            .expect("activation initialized")
            .lock()
            .unwrap()
            .take()
            .expect("one activation subscription");

        cosmic::iced::futures::stream::unfold(receiver, |mut receiver| async {
            receiver.recv().await.map(|request| (request, receiver))
        })
        .map(crate::app::Message::Activated)
    })
}
