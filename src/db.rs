use std::{borrow::Cow, path::Path};

use egui::Color32;
use librespot::core::SpotifyId;

pub struct Connection {
    conn: rusqlite::Connection,
}

impl Connection {
    pub fn open(path: impl AsRef<Path>) -> Self {
        let conn = rusqlite::Connection::open(path).expect("valid connection");
        Self::ensure_table(conn)
    }

    fn ensure_table(conn: rusqlite::Connection) -> Self {
        static SCHEMA: &str = "pragma foreign_keys = 1;
            create table if not exists history (
                spotify_id   blob not null,
                mistake_id   blob not null unique primary key,
                sender_id    text not null,
                sender_name  text not null,
                sender_color blob not null,
                plays        integer not null,
                added_on     blob not null unique,
                deleted      boolean
            );

            create table if not exists queued (
                queue blob unique not null,
                play_order integer unique not null,
                foreign key(queue) references history(mistake_id)
            );";

        conn.execute_batch(SCHEMA).expect("valid sql");

        Self { conn }
    }

    pub fn get_all_history(&self) -> Vec<Item<'static>> {
        self.get_many(
            "select * from history where deleted = false",
            (),
            Item::<'static>::from_row,
        )
    }

    pub fn get_queued(&self) -> Vec<Item<'static>> {
        self.get_many(
            "select * from queued as q
                join history h on h.mistake_id = q.queue;",
            rusqlite::named_params! {},
            Item::from_row,
        )
    }

    // TODO this might be annoying
    pub fn queue<'a>(&self, item: impl Into<Item<'a>> + ?Sized) {
        let Self { conn, .. } = self;

        let item = item.into();
        let mut stmt = conn
            .prepare(
                "insert into queued(queue, play_order)
                    values(:id, (select count(rowid) from queued))",
            )
            .expect("valid sql");

        let _ = stmt.execute(rusqlite::named_params! {":id": item.id});

        self.add_history(item);
    }

    pub fn remove_from_queue<'a>(&self, item: impl Into<Item<'a>> + ?Sized) -> bool {
        let Self { conn, .. } = self;
        let mut stmt = conn
            .prepare("delete from queued where queue = :id")
            .expect("valid sql");

        let item = item.into();
        matches!(
            stmt.execute(rusqlite::named_params! {":id": item.id}),
            Ok(1)
        )
    }

    pub fn update_play_count(&self, id: uuid::Uuid) {
        let Self { conn, .. } = self;
        // TODO just use a view
        let mut stmt = conn
            .prepare(
                "update history set plays =
                    (select plays from history where id = :id) + 1
                    where id = :id;",
            )
            .expect("valid sql");

        let _ = stmt.execute(rusqlite::named_params! {":id": id});
    }

    pub fn add_history<'a>(&self, item: impl Into<Item<'a>> + ?Sized) {
        let Self { conn, .. } = self;
        let mut stmt = conn
            .prepare(
                "insert into history (
                    spotify_id,
                    mistake_id,
                    sender_id,
                    sender_name,
                    sender_color,
                    plays,
                    added_on,
                    deleted
                ) values (
                    :spotify_id,
                    :mistake_id,
                    :sender_id,
                    :sender_name,
                    :sender_color,
                    :plays,
                    :added_on,
                    :deleted
                ) on conflict(mistake_id)
                    do update set plays = plays + 1
                    where deleted = false;",
            )
            .expect("valid sql");

        let item = item.into();
        let _ = stmt.execute(rusqlite::named_params! {
            ":spotify_id": item.spotify_id.to_raw(),
            ":mistake_id": item.id,
            ":sender_id": item.sender_id,
            ":sender_name": item.sender_name,
            ":sender_color": item.sender_color,
            ":plays": item.plays,
            ":added_on": item.added_on,
            ":deleted": false,
        });
    }

    pub fn undelete_item(&self, item: &Item<'_>) -> bool {
        let Self { conn, .. } = self;

        let mut stmt = conn
            .prepare(
                "update history set deleted = false
                    where deleted = true
                    and mistake_id = :mistake_id;",
            )
            .expect("valid sql");

        matches!(
            stmt.execute(rusqlite::named_params! {":mistake_id": item.id}),
            Ok(1)
        )
    }

    pub fn remove_from_history<'a>(&self, item: impl Into<Item<'a>> + ?Sized) -> bool {
        let Self { conn, .. } = self;
        let mut stmt = conn
            .prepare(
                "update history set deleted = true
                    where mistake_id = :mistake_id;",
            )
            .expect("valid sql");

        let item = item.into();
        1 == stmt
            .execute(rusqlite::named_params! {
                ":mistake_id": item.id,
            })
            .expect("valid query")
    }

    pub fn get_queued_ids(&self) -> Vec<uuid::Uuid> {
        self.get_many(
            "select queue from queued
                order by play_order;",
            rusqlite::named_params! {},
            |row| row.get("queue"),
        )
    }

    fn get_many<T>(
        &self,
        sql: &str,
        params: impl rusqlite::Params,
        map: impl Fn(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
    ) -> Vec<T> {
        let Self { conn, .. } = self;
        let mut stmt = conn.prepare(sql).expect("valid sql");
        let resp = stmt.query_map(params, map);
        let Ok(iter) = resp else { return vec![] };
        iter.flatten().collect()
    }
}

#[derive(Debug, Clone)]
pub struct Item<'a> {
    pub id: uuid::Uuid,
    pub spotify_id: SpotifyId,
    pub sender_id: Cow<'a, str>,
    pub sender_name: Cow<'a, str>,
    pub sender_color: u32,
    pub plays: usize,
    pub added_on: time::OffsetDateTime,
}

impl<'a> From<&'a crate::request::Request> for Item<'a> {
    fn from(value: &'a crate::request::Request) -> Self {
        Self {
            id: value.id,
            spotify_id: value.track.id,
            sender_id: Cow::from(value.user.id.as_str()),
            sender_name: Cow::from(value.user.name.as_str()),
            sender_color: Self::convert_color(value.user.color),
            plays: 0,
            added_on: value.added_on,
        }
    }
}

impl<'a> Item<'a> {
    pub fn color_from_u32(color: u32) -> Color32 {
        let (r, g, b) = (
            ((color >> 16) & 0xFF) as _,
            ((color >> 8) & 0xFF) as _,
            (color & 0xFF) as _,
        );

        Color32::from_rgb(r, g, b)
    }

    pub fn convert_color(color: Color32) -> u32 {
        let [r, g, b, _] = color.to_array();
        u32::from_ne_bytes([b, g, r, 0x00])
    }
}

impl Item<'static> {
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get("mistake_id")?,
            spotify_id: SpotifyId::from_raw(&row.get::<_, Vec<u8>>("spotify_id")?)
                .map_err(|_| rusqlite::Error::InvalidQuery)?,
            sender_id: row.get::<_, String>("sender_id")?.into(),
            sender_name: row.get::<_, String>("sender_name")?.into(),
            sender_color: row.get("sender_color")?,
            plays: row.get("plays")?,
            added_on: row.get("added_on")?,
        })
    }
}
