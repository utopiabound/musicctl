// Copyright 2023 (c) Nathaniel Clark

mod mpris;
mod radiotray;
mod shairportsync;

use async_trait::async_trait;
use futures::future::try_join_all;
use std::collections::HashMap;
use thiserror::Error;
use zbus::{proxy, Connection};
use zvariant::Value;

#[derive(Debug, Error)]
pub(crate) enum McError {
    #[error(transparent)]
    Zbus(#[from] zbus::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("No active players avaiable")]
    NoActive,
}

#[async_trait]
pub(crate) trait MusicCtl {
    // play/pause
    async fn mc_play(&self) -> Result<(), McError>;
    async fn mc_stop(&self) -> Result<(), McError>;
    async fn mc_name(&self) -> Result<String, McError>;
    async fn mc_info(&self) -> Result<Option<MusicInfo>, McError>;
    async fn mc_next(&self) -> Result<(), McError>;
    async fn mc_prev(&self) -> Result<(), McError>;
    async fn mc_canplay(&self) -> Result<bool, McError>;
}

#[derive(Debug, Clone)]
pub(crate) struct MusicInfo {
    pub artist: String,
    pub title: String,
    pub album: String,
    pub cover: String,
}

impl std::fmt::Display for MusicInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if !self.album.is_empty() {
            write!(f, "'{}' ", self.album)?;
        }
        if !self.title.is_empty() {
            write!(f, "{} by ", self.title)?;
        }
        write!(f, "{}", self.artist)
    }
}

impl TryFrom<HashMap<String, Value<'_>>> for MusicInfo {
    type Error = McError;

    fn try_from(xs: HashMap<String, Value>) -> Result<Self, Self::Error> {
        Ok(MusicInfo {
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
        })
    }
}

#[proxy(assume_defaults = true)]
trait DBus {
    fn list_names(&self) -> zbus::Result<Vec<String>>;
}

pub(crate) fn get_json_string(xs: &serde_json::Value, key: &str) -> String {
    xs.get(key)
        .and_then(|x| x.as_str())
        .map(|x| x.to_string())
        .unwrap_or_default()
}

pub(crate) fn variant_val_to_string(x: &zbus::zvariant::Value) -> String {
    match x {
        zbus::zvariant::Value::Str(s) => s.to_string(),
        zbus::zvariant::Value::Array(a) => variant_val_to_string(&a[0]),
        _ => todo!(),
    }
}

pub(crate) async fn get_all(conn: &Connection) -> Result<Vec<Box<dyn MusicCtl>>, McError> {
    let proxy = DBusProxy::new(conn).await?;

    let xs = proxy.list_names().await?;

    let mut list: Vec<Box<dyn MusicCtl>> = try_join_all(
        xs.iter()
            .filter(|x| x.starts_with(mpris::MPRIS_PREFIX))
            .map(|x| {
                let conn = conn.clone();
                async move {
                    match x.as_str() {
                        shairportsync::SERVICE_NAME => {
                            shairportsync::ShairportSyncProxy::builder(&conn)
                                .destination(x.to_string())?
                                .build()
                                .await
                                .map(|x| Box::new(x) as Box<dyn MusicCtl>)
                        }
                        _ => mpris::Mpris2Proxy::builder(&conn)
                            .destination(x.to_string())?
                            .build()
                            .await
                            .map(|x| Box::new(x) as Box<dyn MusicCtl>),
                    }
                }
            }),
    )
    .await?;

    if xs.contains(&radiotray::RADIOTRAY_NG.to_string()) {
        let x = radiotray::RadioTrayNGProxy::builder(conn).build().await?;
        list.push(Box::new(x));
    }

    Ok(list)
}
