// Copyright 2023 (c) Nathaniel Clark

use crate::plugin::{get_json_string, McError, MusicCtl, MusicInfo};
use async_trait::async_trait;
use serde_json::Value;
use zbus::dbus_proxy;

pub(crate) const RADIOTRAY_NG: &str = "com.github.radiotray_ng";

#[dbus_proxy(
    interface = "com.github.radiotray_ng",
    default_service = "com.github.radiotray_ng",
    default_path = "/com/github/radiotray_ng"
)]
trait RadioTrayNG {
    #[dbus_proxy(name = "play")]
    fn play(&self) -> zbus::Result<()>;
    #[dbus_proxy(name = "mute")]
    fn mute(&self) -> zbus::Result<()>;
    #[dbus_proxy(name = "stop")]
    fn stop(&self) -> zbus::Result<()>;
    #[dbus_proxy(name = "next_station")]
    fn next_station(&self) -> zbus::Result<()>;
    #[dbus_proxy(name = "previous_station")]
    fn previous_station(&self) -> zbus::Result<()>;
    // returns quoted json
    #[dbus_proxy(name = "get_player_state")]
    fn get_player_state(&self) -> zbus::Result<String>;
}

#[async_trait]
impl MusicCtl for RadioTrayNGProxy<'_> {
    async fn mc_play(&self) -> Result<(), McError> {
        self.play().await?;
        Ok(())
    }
    async fn mc_stop(&self) -> Result<(), McError> {
        self.stop().await?;
        Ok(())
    }
    async fn mc_name(&self) -> Result<String, McError> {
        Ok("RadioTrayNG".to_string())
    }
    async fn mc_info(&self) -> Result<Option<MusicInfo>, McError> {
        let xs: Value = serde_json::from_str(self.get_player_state().await?.as_str())?;

        if xs.is_null() {
            Ok(None)
        } else {
            Ok(Some(MusicInfo {
                artist: get_json_string(&xs, "artist"),
                title: get_json_string(&xs, "title"),
                album: get_json_string(&xs, "station"),
                cover: "".to_string(),
            }))
        }
    }
    async fn mc_next(&self) -> Result<(), McError> {
        self.next_station().await?;
        Ok(())
    }
    async fn mc_prev(&self) -> Result<(), McError> {
        self.previous_station().await?;
        Ok(())
    }
    async fn mc_canplay(&self) -> Result<bool, McError> {
        Ok(
            serde_json::from_str::<Value>(self.get_player_state().await?.as_str())?
                .get("url")
                .and_then(|x| x.as_str())
                .map(|x| !x.is_empty())
                .unwrap_or_default(),
        )
    }
}
