// Copyright 2023 (c) Nathaniel Clark

use async_trait::async_trait;
use clap::{Parser, ValueEnum};
use futures::{future::{try_join_all, join_all}, TryFutureExt};
use std::{collections::HashMap, process::ExitCode};
use thiserror::Error;
use zbus::{dbus_proxy, zvariant::Value, Connection};

/// Looks for running music player and issues appropriate command to it
#[derive(Debug, Default, Clone, Parser)]
struct App {
    #[clap(long, short)]
    instance: Option<String>,

    #[clap(value_enum, default_value_t)]
    command: Command,
}

#[derive(Debug, Default, Clone, Copy, ValueEnum)]
enum Command {
    List,
    Play,
    Stop,
    Next,
    Prev,
    #[default]
    Info,
    Vinfo,
    Mute,
}

#[derive(Debug, Error)]
pub enum McError {
    #[error(transparent)]
    ZbusError(#[from] zbus::Error),

    #[error("No active players avaiable")]
    NoActive,
}    

#[dbus_proxy(assume_defaults = true)]
trait Notifications {
    /// Call the org.freedesktop.Notifications.Notify D-Bus method
    #[allow(clippy::too_many_arguments)]
    fn notify(
        &self,
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: &[&str],
        hints: HashMap<&str, &Value<'_>>,
        expire_timeout: i32,
    ) -> zbus::Result<u32>;
}

#[dbus_proxy(assume_defaults = true)]
trait DBus {
    fn list_names(&self) -> zbus::Result<Vec<String>>;
}

#[dbus_proxy(
    interface = "org.mpris.MediaPlayer2.Player",
    default_service = "org.mpris.MediaPlayer2",
    default_path = "/org/mpris/MediaPlayer2"
)]
trait Mpris2 {
    fn play_pause(&self) -> zbus::Result<()>;
    fn next(&self) -> zbus::Result<()>;
    fn previous(&self) -> zbus::Result<()>;
    fn stop(&self) -> zbus::Result<()>;
    // returns xml
    #[dbus_proxy(property)]
    fn metadata(&self) -> zbus::Result<HashMap<String, zbus::zvariant::Value>>;
}

#[derive(Debug, Clone)]
struct MusicInfo {
    artist: String,
    title: String,
    album: String,
    cover: String,
}
impl std::fmt::Display for MusicInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.album != "" {
            write!(f, "'{}' ", self.album)?;
        }
        if self.title != "" {
            write!(f, "{} by ", self.title)?;
        }
        write!(f, "{}", self.artist)
    }
}

fn variant_val_to_str(x: &zbus::zvariant::Value) -> String {
    match x {
        zbus::zvariant::Value::Str(s) => s.to_string(),
        zbus::zvariant::Value::Array(a) => variant_val_to_str(&a[0]),
        _ => todo!(),
    }
}

#[async_trait]
impl MusicCtl for Mpris2Proxy<'_> {
    async fn mc_play(&self) -> Result<(), McError> {
        self.play_pause().await?;
        Ok(())
    }
    async fn mc_stop(&self) -> Result<(), McError> {
        self.stop().await?;
        Ok(())
    }
    async fn mc_name(&self) -> Result<String, McError> {
        let name = &self.inner().destination().as_str()["org.mpris.MediaPlayer2.".len()..];
        Ok(format!("{name} (MPRIS)"))
    }
    async fn mc_info(&self) -> Result<Option<MusicInfo>, McError> {
        let xs = self.metadata().await?;
        if xs.is_empty() {
            Ok(None)
        } else {
            Ok(Some(MusicInfo {
                artist: xs.get("xesam:artist").map(variant_val_to_str).unwrap_or_default(),
                title: xs.get("xesam:title").map(variant_val_to_str).unwrap_or_default(),
                album: xs.get("xesam:album").map(variant_val_to_str).unwrap_or_default(),
                cover: xs.get("mpris:artUrl").map(variant_val_to_str).unwrap_or_default(),
            }))
        }
    }
    async fn mc_next(&self) -> Result<(), McError> {
        self.next().await?;
        Ok(())
    }
    async fn mc_prev(&self) -> Result<(), McError> {
        self.previous().await?;
        Ok(())
    }
    async fn mc_canplay(&self) -> Result<bool, McError> {
        Ok(self.metadata().await?.get("xesam:artist").is_some())
    }
}

#[async_trait]
trait MusicCtl {
    // play/pause
    async fn mc_play(&self) -> Result<(), McError>;
    async fn mc_stop(&self) -> Result<(), McError>;
    async fn mc_name(&self) -> Result<String, McError>;
    async fn mc_info(&self) -> Result<Option<MusicInfo>, McError>;
    async fn mc_next(&self) -> Result<(), McError>;
    async fn mc_prev(&self) -> Result<(), McError>;
    async fn mc_canplay(&self) -> Result<bool, McError>;
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {e}");

            ExitCode::FAILURE
        }
    }
}


async fn run() -> Result<(), McError> {
    let cmd = App::parse();
    let session = Connection::session().await?;

    let proxy = DBusProxy::new(&session).await?;
    let xs = proxy.list_names().await?;

    let mpris_list = try_join_all(
        xs.into_iter()
            .filter(|x| x.starts_with("org.mpris.MediaPlayer2."))
            .map(|x| {
                let session = session.clone();
                async move {
                    Mpris2Proxy::builder(&session)
                        .destination(x)?
                        .build()
                        .await
                }})
    ).await?;

    let list = (if let Some(name) = cmd.instance {
        join_all(mpris_list.iter().map(|x| {
            let name = name.clone();
            async move {
                if x.mc_name().await.unwrap_or_default() == name {
                    (x, x.mc_info().await.unwrap_or_default())
                } else {
                    (x, None)
                }
            }
        })).await
    } else {
        join_all(mpris_list.iter().map(|x| async move { (x, x.mc_info().await.unwrap_or_default()) })).await
    }
    ).into_iter().filter_map(|(x, info)| info.map(|i| (x, i))).collect::<Vec<_>>();

    if list.is_empty() {
        return Err(McError::NoActive);
    }
    
    match cmd.command {
        Command::List => {
            for m in list {
                println!("{}: {}", m.0.mc_name().await?, m.1);
            }
        }
        Command::Info => println!("{}: {}", list[0].0.mc_name().await?, list[0].1),
        Command::Play => list[0].0.mc_play().await?,
        Command::Stop => list[0].0.mc_stop().await?,
        Command::Next => list[0].0.mc_next().await?,
        Command::Prev => list[0].0.mc_prev().await?,
        Command::Vinfo => {
            let proxy = NotificationsProxy::new(&session).await?;
            let id = proxy
                .notify(
                    env!("CARGO_PKG_NAME"),
                    0,
                    &list[0].1.cover,
                    &list[0].1.to_string(),
                    &list[0].0.mc_name().await?,
                    &[],
                    HashMap::new(),
                    0,
                )
                .await?;

            println!("Created Notification: {id}");
        }
        Command::Mute => todo!(),
    }
    Ok(())
}
