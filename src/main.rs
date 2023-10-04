// Copyright 2023 (c) Nathaniel Clark

mod plugin;

use crate::plugin::{get_all, McError, MusicCtl};

use clap::{Parser, ValueEnum};
use futures::future::join_all;
use std::{collections::HashMap, process::ExitCode};
use zbus::{dbus_proxy, zvariant::Value, Connection};

/// Looks for running music player and issues appropriate command to it
#[derive(Debug, Default, Clone, Parser)]
struct App {
    #[clap(long, short)]
    debug: bool,

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

async fn first_active<'a>(
    name: &'a Option<String>,
    list: &'a [Box<dyn MusicCtl>],
) -> Result<&'a Box<dyn MusicCtl>, McError> {
    for item in list {
        if item.mc_canplay().await? {
            if let Some(name) = name {
                if &item.mc_name().await? == name {
                    return Ok(item);
                }
            } else {
                return Ok(item);
            }
        }
    }
    Err(McError::NoActive)
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

    let list = get_all(&session).await?;

    let active = first_active(&cmd.instance, &list).await?;

    match cmd.command {
        Command::List => {
            join_all(list.iter().map(|x| async move {
                if cmd.debug {
                    println!(
                        "{}: {:?}",
                        x.mc_name().await.unwrap_or_default(),
                        x.mc_info().await
                    );
                } else if let Some(info) = x.mc_info().await.unwrap_or_default() {
                    println!("{}: {}", x.mc_name().await.unwrap_or_default(), info);
                }
            }))
            .await;
        }
        Command::Info => {
            if let Some(info) = active.mc_info().await? {
                println!("{}: {}", active.mc_name().await?, info);
            }
        }
        Command::Play => active.mc_play().await?,
        Command::Stop => active.mc_stop().await?,
        Command::Next => active.mc_next().await?,
        Command::Prev => active.mc_prev().await?,
        Command::Vinfo => {
            if let Some(info) = active.mc_info().await.unwrap_or_default() {
                let proxy = NotificationsProxy::new(&session).await?;
                let id = proxy
                    .notify(
                        env!("CARGO_PKG_NAME"),
                        0,
                        &info.cover,
                        &info.to_string(),
                        &active.mc_name().await?,
                        &[],
                        HashMap::new(),
                        0,
                    )
                    .await?;
                println!("Created Notification: {id}");
            }
        }
        Command::Mute => todo!(),
    }
    Ok(())
}
