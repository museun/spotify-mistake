use egui::{TextureHandle, TextureId, TextureOptions};
use hashbrown::{hash_map::Entry, HashMap};
use librespot::core::{session::Session, FileId};

use crate::async_adapter::{defer_repaint, Fut, Ready};

pub struct ImageCache {
    map: hashbrown::HashMap<FileId, Ready<TextureHandle>>,
    pending: Vec<Fut<(FileId, Option<TextureHandle>)>>,
    session: Session,
    ctx: egui::Context,
}

impl ImageCache {
    pub fn new(session: Session, ctx: egui::Context) -> Self {
        Self {
            map: HashMap::default(),
            pending: vec![],
            session,
            ctx,
        }
    }

    pub fn get(&mut self, file_id: FileId) -> Option<TextureId> {
        match self.map.entry(file_id) {
            Entry::Occupied(entry) => {
                if let Ready::Ready(item) = entry.get() {
                    return Some(item.id());
                }
            }
            Entry::Vacant(entry) => {
                entry.insert(Ready::NotReady);
                let fut = Self::fetch(self.ctx.clone(), file_id, self.session.clone());
                self.pending.push(fut);
            }
        }
        None
    }

    pub fn poll(&mut self) {
        self.pending.retain_mut(|fut| {
            let Some((k, Some(v))) = fut.resolve() else { return true };
            *self.map.get_mut(&k).unwrap() = Ready::Ready(v);
            false
        });
    }

    fn fetch(
        ctx: egui::Context,
        file_id: FileId,
        session: Session,
    ) -> Fut<(FileId, Option<TextureHandle>)> {
        fn load_texture(
            ctx: &egui::Context,
            name: &str,
            data: &[u8],
        ) -> anyhow::Result<TextureHandle> {
            let img = ::image::load_from_memory(data)?;
            let data = img.to_rgba8();
            let (width, height) = data.dimensions();
            let image = egui::ColorImage::from_rgba_unmultiplied([width as _, height as _], &data);
            let handle = ctx.load_texture(name, image, TextureOptions::default());
            Ok(handle)
        }

        Fut::spawn(async move {
            let _d = defer_repaint(&ctx);
            match session.spclient().get_image(&file_id).await {
                Ok(data) => match ::image::guess_format(&data[..64.min(data.len())]) {
                    Ok(fmt) => match fmt {
                        ::image::ImageFormat::Jpeg | ::image::ImageFormat::Png => {
                            match load_texture(&ctx, &file_id.to_string(), &data) {
                                Ok(handle) => return (file_id, Some(handle)),
                                Err(err) => log::warn!("{file_id} cannot load texture: {err}"),
                            };
                        }
                        fmt => log::warn!("{file_id} unknown format: {fmt:?}"),
                    },
                    Err(err) => log::warn!("{file_id} cannot guess image format: {err}"),
                },
                Err(err) => log::warn!("{file_id} is not a valid image: {err}"),
            };
            (file_id, None)
        })
    }
}
