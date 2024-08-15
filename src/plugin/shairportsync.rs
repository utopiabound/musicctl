// Copyright 2024 (c) Nathaniel Clark

use crate::plugin::{McError, MusicCtl, MusicInfo};
use async_trait::async_trait;
use std::collections::HashMap;
use zbus_macros::proxy;
use zvariant::Value;

pub const SERVICE_NAME: &str = "org.mpris.MediaPlayer2.ShairportSync";

#[proxy(
    interface = "org.gnome.ShairportSync.RemoteControl",
    default_service = "org.mpris.MediaPlayer2.ShairportSync",
    default_path = "/org/gnome/ShairportSync"
)]
trait ShairportSync {
    fn play_pause(&self) -> zbus::Result<()>;
    fn stop(&self) -> zbus::Result<()>;
    fn next(&self) -> zbus::Result<()>;
    fn previous(&self) -> zbus::Result<()>;
    // returns xml
    #[zbus(property)]
    fn metadata(&self) -> zbus::Result<HashMap<String, Value>>;
    #[zbus(property)]
    fn available(&self) -> zbus::Result<bool>;
}

#[async_trait]
impl MusicCtl for ShairportSyncProxy<'_> {
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
        Ok(name.to_string())
    }
    async fn mc_info(&self) -> Result<Option<MusicInfo>, McError> {
        let xs = self.metadata().await?;
        if xs.is_empty() {
            Ok(None)
        } else {
            Ok(Some(xs.try_into()?))
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
        Ok(self.available().await? && self.metadata().await?.contains_key("xesam:artist"))
    }
}
