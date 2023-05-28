use std::{borrow::Cow, collections::BTreeMap, sync::Arc};

use librespot::{
    core::{Session, SpotifyId},
    metadata::{image::ImageSize, Lyrics, Metadata as _, Track},
};
use tokio::task::JoinSet;

use crate::{async_adapter::Fut, spotify_lyrics::SpotifyLyrics, twitch, Request};

#[derive(Default)]
pub struct History {
    pub requests: Vec<Request>,
}

impl History {
    pub fn load(data: &str, session: &Session) -> Fut<Self> {
        #[derive(::serde::Deserialize, Default)]
        struct HistoryLoad {
            queue: Vec<HistoryItem<'static>>,
        }

        Fut {
            fut: lookup_all_requests(
                serde_json::from_str::<HistoryLoad>(data)
                    .unwrap_or_default()
                    .queue,
                session.clone(),
                |requests| Self { requests },
            ),
            resolved: false,
        }
    }

    pub fn save(&self) -> String {
        #[derive(::serde::Serialize)]
        struct HistorySave<'a> {
            queue: Vec<HistoryItem<'a>>,
        }

        serde_json::to_string(&HistorySave {
            queue: self.requests.iter().map(HistoryItem::map).collect(),
        })
        .unwrap()
    }
}

#[derive(::serde::Serialize, ::serde::Deserialize)]
pub struct HistoryItem<'a> {
    #[serde(with = "serde::spotify_id")]
    pub id: SpotifyId,
    pub user: Cow<'a, twitch::User>,
}

impl HistoryItem<'static> {
    pub async fn lookup(self, session: &Session) -> Request {
        let track = Track::get(session, &self.id).await.map(Arc::new).unwrap();

        let lyrics = Lyrics::get(session, &self.id).await.ok();
        let lyrics = lyrics
            .map(|lyrics| SpotifyLyrics::fix_up(lyrics, track.duration as _))
            .unwrap_or_default();

        let image_id = track
            .album
            .covers
            .iter()
            .find_map(|c| (c.size == ImageSize::DEFAULT).then_some(c.id));

        Request {
            image_id,
            track,
            user: self.user.into_owned(),
            lyrics,
        }
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

            let deque = tree.into_values().collect();
            let _ = tx.send(map(deque));
        }
    });

    rx
}

impl<'a> HistoryItem<'a> {
    pub fn map(request: &'a Request) -> Self {
        Self {
            id: request.track.id,
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
            serializer.serialize_str(
                &id.to_uri()
                    .map_err(|err| S::Error::custom(err.to_string()))?,
            )
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
