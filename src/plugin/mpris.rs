// Copyright 2023 (c) Nathaniel Clark

use crate::plugin::{variant_val_to_string, McError, MusicCtl, MusicInfo};
use async_trait::async_trait;
use std::collections::HashMap;
use zbus::dbus_proxy;

pub(crate) const MPRIS_PREFIX: &str = "org.mpris.MediaPlayer2.";

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
                artist: xs
                    .get("xesam:artist")
                    .map(variant_val_to_string)
                    .unwrap_or_default(),
                title: xs
                    .get("xesam:title")
                    .map(variant_val_to_string)
                    .unwrap_or_default(),
                album: xs
                    .get("xesam:album")
                    .map(variant_val_to_string)
                    .unwrap_or_default(),
                cover: xs
                    .get("mpris:artUrl")
                    .map(variant_val_to_string)
                    .unwrap_or_default(),
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
