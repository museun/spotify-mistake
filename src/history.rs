use std::{borrow::Cow, collections::BTreeMap, sync::Arc};

use librespot::{
    core::{Session, SpotifyId},
    metadata::{image::ImageSize, Lyrics, Metadata as _, Track},
};
use tokio::{sync::mpsc::UnboundedSender, task::JoinSet};

use crate::{
    async_adapter::Fut, bot::SynthEvent, db, spotify_lyrics::SpotifyLyrics, twitch, Request,
};

#[derive(Default)]
pub struct History {
    pub requests: Vec<Request>,
}

impl History {
    pub fn load(
        session: &Session,
        db: &db::Connection,
        replay: UnboundedSender<SynthEvent<Request>>,
    ) -> Fut<Self> {
        use twitch_message::messages::types::{Nickname, UserId};

        let map_db_item = |item: db::Item<'static>| HistoryItem {
            id: item.id,
            spotify_id: item.spotify_id,
            user: Cow::Owned(twitch::User {
                id: UserId::from(item.sender_id.to_string()),
                name: Nickname::from(item.sender_name.to_string()),
                color: db::Item::color_from_u32(item.sender_color),
            }),
            added_on: item.added_on,
        };

        let history_items = db.get_all_history().into_iter().map(map_db_item);
        let queue_items = db.get_queued().into_iter().map(map_db_item);

        Fut::spawn({
            let session = session.clone();
            async move {
                let history = lookup_all_requests(history_items, session.clone(), |requests| {
                    Self { requests }
                });
                let queue = lookup_all_requests(queue_items, session, move |items| {
                    items
                        .into_iter()
                        .map(SynthEvent::Synthetic)
                        .try_for_each(|item| replay.send(item))
                        .ok()
                        .expect("control state incontinuity");
                });
                let (history, _) = tokio::join!(history, queue);
                history.expect("load history")
            }
        })
    }
}

#[derive(::serde::Serialize, ::serde::Deserialize)]
pub struct HistoryItem<'a> {
    pub id: uuid::Uuid,
    #[serde(with = "serde::spotify_id")]
    pub spotify_id: SpotifyId,
    pub user: Cow<'a, twitch::User>,
    pub added_on: time::OffsetDateTime,
}

impl HistoryItem<'static> {
    pub async fn lookup(self, session: &Session) -> Option<Request> {
        let track = Track::get(session, &self.spotify_id)
            .await
            .map(Arc::new)
            .ok()?;
        let request = Request {
            id: self.id,
            added_on: self.added_on,
            image_id: track
                .album
                .covers
                .iter()
                .find_map(|c| (c.size == ImageSize::DEFAULT).then_some(c.id)),
            user: self.user.into_owned(),
            lyrics: Lyrics::get(session, &self.spotify_id)
                .await
                .ok()
                .map(|lyrics| SpotifyLyrics::fix_up(lyrics, track.duration as _))
                .unwrap_or_default(),
            track,
        };
        Some(request)
    }
}

pub fn lookup_all_requests<T>(
    iterator: impl IntoIterator<Item = HistoryItem<'static>> + Send + Sync + 'static,
    session: Session,
    map: impl FnOnce(Vec<Request>) -> T + Send + Sync + 'static,
) -> tokio::sync::oneshot::Receiver<T>
where
    T: Send + Sync + 'static,
{
    let (tx, rx) = tokio::sync::oneshot::channel();

    tokio::spawn({
        async move {
            let mut set = JoinSet::new();
            // TODO limit this with a semaphore
            for (id, item) in iterator.into_iter().enumerate() {
                let session = session.clone();
                let fut = async move {
                    let request = item.lookup(&session).await;
                    (id, request)
                };
                set.spawn(fut);
            }

            let mut tree = BTreeMap::default();
            while let Some(Ok((id, req))) = set.join_next().await {
                tree.insert(id, req);
            }

            let deque = tree.into_values().flatten().collect();
            let _ = tx.send(map(deque));
        }
    });

    rx
}

impl<'a> HistoryItem<'a> {
    pub fn map(request: &'a Request) -> Self {
        Self {
            id: request.id,
            added_on: request.added_on,
            spotify_id: request.track.id,
            user: Cow::Borrowed(&request.user),
        }
    }
}

mod serde {
    pub mod spotify_id {
        use librespot::core::SpotifyId;

        pub fn serialize<S>(id: &SpotifyId, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: ::serde::Serializer,
        {
            use serde::ser::Error as _;
            let data = id
                .to_uri()
                .map_err(|err| S::Error::custom(err.to_string()))?;
            serializer.serialize_str(&data)
        }

        pub fn deserialize<'de, D>(deserializer: D) -> Result<SpotifyId, D::Error>
        where
            D: ::serde::Deserializer<'de>,
        {
            use ::serde::de::{Deserialize as _, Error as _};

            let s = <std::borrow::Cow<'_, str>>::deserialize(deserializer)?;
            SpotifyId::from_uri(&s).map_err(|err| D::Error::custom(err.to_string()))
        }
    }
}
