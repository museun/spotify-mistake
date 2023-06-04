use std::{collections::VecDeque, time::Duration};

use egui::{Align, CentralPanel, FontDefinitions, FontTweak, Layout, Slider, TextStyle, Visuals};

use librespot::{
    core::session::Session,
    playback::player::{Player, PlayerEventChannel},
};

use tokio::sync::{
    mpsc::{UnboundedReceiver, UnboundedSender},
    oneshot,
};

use crate::{
    async_adapter::Fut,
    bot::SynthEvent,
    db,
    ext::JoinWith,
    history::History,
    image_cache::ImageCache,
    player_state::{NextPlayingState, PlayerState},
    request::Request,
    tab_selection::TabSelection,
    views::HistoryView,
    views::{ImageView, QueueView},
    views::{ListView, RequestView},
    volume_state::VolumeState,
};

use self::{active_control::ActiveControl, info_panel::InfoPanel};

mod active_control;
mod info_panel;
mod lyrics_panel;
mod player_control;

struct Active {
    play_pos: Option<Duration>,
    request: Request,
}

pub struct Control {
    cache: ImageCache,
    active: Option<Active>,
    queue: VecDeque<Request>,

    history: History,
    history_fut: Fut<History>,
    out_of_band: Vec<Request>,

    events: UnboundedReceiver<SynthEvent<Request>>,
    requests: UnboundedReceiver<oneshot::Sender<Option<Request>>>,

    player: Player,
    player_state: PlayerState,
    player_events: PlayerEventChannel,
    next_playing: NextPlayingState,

    state: ControlState,

    tab_view: TabSelection,

    db: db::Connection,
}

impl Control {
    pub fn create(
        cc: &eframe::CreationContext,
        session: Session,
        player: Player,
        volume: VolumeState,
        replay: UnboundedSender<SynthEvent<Request>>,
        events: UnboundedReceiver<SynthEvent<Request>>,
        requests: UnboundedReceiver<oneshot::Sender<Option<Request>>>,
    ) -> Box<dyn eframe::App> {
        cc.egui_ctx.set_pixels_per_point(2.0);

        // TODO get this from the configuration (or just use dirs)
        let db = db::Connection::open("history.db");
        Self::load_fonts(&cc.egui_ctx);

        let history_fut = History::load(&session, &db, replay);

        let state = cc
            .storage
            .map(|storage| ControlState::load(storage, volume.clone()))
            .unwrap_or_else(|| ControlState {
                volume,
                always_on_top: false,
                auto_play: false,
            });

        Box::new(Self {
            cache: ImageCache::new(session, cc.egui_ctx.clone()),
            active: None,
            queue: VecDeque::new(),

            history: History::default(),
            history_fut,
            out_of_band: Vec::new(),

            events,
            requests,

            player_events: player.get_player_event_channel(),
            player,
            player_state: PlayerState::default(),

            next_playing: NextPlayingState::default(),

            state,

            tab_view: TabSelection::default(),

            db,
        })
    }

    fn populate_from_db(
        db: &db::Connection,
        session: &Session,
        replay: UnboundedSender<SynthEvent<Request>>,
    ) {
        let history = db.get_all_history();
        let queued = db.get_queued();
    }

    fn load_fonts(ctx: &egui::Context) {
        let mut fonts = FontDefinitions::empty();
        macro_rules! load_font {
            ($($font:expr => $entry:expr),*) => {
                $(
                    fonts.font_data.insert($font.into(), egui::FontData::from_static(
                        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),"/fonts/", $font))
                    ));
                    fonts.families.entry($entry).or_default().push($font.into());
                )*
            };
        }

        // TODO support rtl languages as well

        load_font! {
            "NotoSans-Regular.ttf"   => egui::FontFamily::Proportional,
            "NotoSansJP-Regular.ttf" => egui::FontFamily::Proportional,
            "NotoSansKR-Regular.otf" => egui::FontFamily::Proportional,
            "NotoSansSC-Regular.otf" => egui::FontFamily::Proportional,
            "NotoSansTC-Regular.otf" => egui::FontFamily::Proportional,
            "NotoEmoji-Regular.ttf"  => egui::FontFamily::Proportional,
            // TODO get a different font
            "NotoSans-Regular.ttf"   => egui::FontFamily::Monospace
        }

        let tweak = FontTweak {
            y_offset: 1.2,
            ..FontTweak::default()
        };

        fonts
            .font_data
            .get_mut("NotoEmoji-Regular.ttf")
            .unwrap()
            .tweak = tweak;

        ctx.set_fonts(fonts);
    }

    fn display_tab_list(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for tab_view in [TabSelection::Queue, TabSelection::History] {
                ui.selectable_value(&mut self.tab_view, tab_view, tab_view.label());
            }
        });

        let list_view = ListView {
            cache: &mut self.cache,
        };

        match self.tab_view {
            TabSelection::Queue => {
                QueueView {
                    list_view,
                    queue: &mut self.queue,
                    db: &self.db,
                }
                .display(ui);
            }
            TabSelection::History => {
                HistoryView {
                    list_view,
                    queue: &mut self.queue,
                    history: &mut self.history,
                    db: &self.db,
                }
                .display(ui);
            }
        }
    }

    fn read_events(&mut self) {
        while let Ok(req) = self.events.try_recv() {
            let req = match req {
                SynthEvent::Synthetic(req) => req,
                SynthEvent::Organic(req) => {
                    let place = if self.history_fut.is_resolved() {
                        self.db.add_history(&req);
                        &mut self.history.requests
                    } else {
                        &mut self.out_of_band
                    };
                    place.push(req.clone());
                    req
                }
            };

            self.db.queue(&req);
            if self.active.is_none() {
                self.active.replace(Active {
                    play_pos: None,
                    request: req,
                });
                continue;
            }
            self.queue.push_back(req);
        }
    }

    // this is how the bot requests the current song
    // TODO make this way less obscure
    fn read_requests(&mut self) {
        while let Ok(resp) = self.requests.try_recv() {
            let _ = resp.send(self.active.as_mut().map(|c| c.request.clone()));
        }
    }

    fn read_state(&mut self) {
        while let Ok(event) = self.player_events.try_recv() {
            if let Ok(state) = PlayerState::try_from(event) {
                self.player_state = state;
            }
        }
    }

    fn handle_key_presses(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if ctx.input(|i| i.key_pressed(egui::Key::F12)) {
            ctx.set_debug_on_hover(!ctx.debug_on_hover())
        }

        if ctx.input(|i| i.key_pressed(egui::Key::F)) {
            self.state.always_on_top = !self.state.always_on_top;
            frame.set_always_on_top(self.state.always_on_top);
        }

        if ctx.input(|i| i.key_pressed(egui::Key::T)) {
            let visuals = if ctx.style().visuals.dark_mode {
                Visuals::light()
            } else {
                Visuals::dark()
            };

            ctx.set_visuals(visuals);
        }
    }

    // TODO this replace stuff is inane
    fn display_active(&mut self, ui: &mut egui::Ui, replace: &mut Option<Request>) {
        ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
            let has_active = self.active.is_some();
            let Active { request, play_pos } = match &mut self.active {
                Some(active) => active,
                None => {
                    if let Some(item) = self.queue.front().cloned() {
                        ui.vertical(|ui| {
                            let fid = TextStyle::Heading.resolve(ui.style());
                            let space = ui.fonts(|f| f.glyph_width(&fid, ' '));

                            ui.horizontal(|ui| {
                                ImageView {
                                    texture_id: item.image_id.and_then(|fid| self.cache.get(fid)),
                                    size: 64.0,
                                }
                                .display(ui);

                                RequestView {
                                    request: &item,
                                    fid: &fid,
                                    space: 0.0,
                                    active: ui.visuals().strong_text_color(),
                                    inactive: ui.visuals().text_color(),
                                }
                                .display(ui);
                            });

                            player_control::PlayerControl {
                                has_active,
                                player_state: &self.player_state,
                                player: &mut self.player,
                                request: &item,
                                queue: &mut self.queue,
                                auto_play: &mut self.state.auto_play,
                                volume: &mut self.state.volume,
                            }
                            .display(ui, replace);
                        });
                        return;
                    }

                    ui.vertical(|ui| {
                        ui.heading("nothing in queue, add something");
                        ui.horizontal(|ui| {
                            ui.toggle_value(&mut self.state.auto_play, "Auto");
                            let mut vol = self.state.volume.volume.lock();
                            ui.add(
                                Slider::new(&mut *vol, 0.0..=1.0)
                                    .step_by(0.01)
                                    .trailing_fill(true)
                                    .show_value(false),
                            )
                            .on_hover_text(format!("Volume factor: {vol:.2?}", vol = *vol));
                        });
                    });
                    return;
                }
            };

            let elapsed = play_pos.map(|s| s.as_millis() as usize);

            let resp = ActiveControl {
                request,
                play_pos,
                elapsed,
                has_active,
                cache: &mut self.cache,
                queue: &mut self.queue,
                auto_play: &mut self.state.auto_play,
                player: &mut self.player,
                player_state: &self.player_state,
                volume: &self.state.volume,
            }
            .display(ui, replace);

            InfoPanel {
                request,
                cache: &mut self.cache,
                player: &self.player,
                elapsed,
                height: resp.rect.height(),
            }
            .display(ui);
        });
    }

    fn handle_replace(&mut self, replace: Option<Request>) {
        let Some(request) = replace else { return };

        if let Some(Active { request, .. }) = self.active.replace(Active {
            play_pos: None,
            request,
        }) {
            self.db.remove_from_queue(&request);
        }

        self.player.stop();
        let _ = std::mem::take(&mut self.next_playing);

        if !self.state.auto_play {
            return;
        }

        let Some(Active { request, .. }) = &self.active else { return };

        log::info!(
            "playing: {name} by {artist} requested by: \
            {user} ({user_id})",
            name = request.track.name,
            artist = request.track.artists.iter().map(|c| &c.name).join(", "),
            user = request.user.name,
            user_id = request.user.id,
        );

        self.player.load(request.track.id, true, 0);
        self.player.play();
    }

    fn check_state(&mut self, replace: &mut Option<Request>) {
        match &self.player_state {
            PlayerState::Playing { .. } => {
                self.next_playing = NextPlayingState::Playing;
            }
            PlayerState::EndOfPlaying { .. }
                if matches!(self.next_playing, NextPlayingState::Playing)
                    && self.state.auto_play =>
            {
                if let Some(Active { play_pos, .. }) = &mut self.active {
                    let _ = play_pos.take();
                }

                *replace = self.queue.pop_front();
            }
            PlayerState::PreloadNextTrack { id, req_id } => {
                if let Some(req) = self.queue.front() {
                    self.player.preload(req.track.id);
                }
                if let Some(active) = &self.active {
                    self.player_state = PlayerState::Playing {
                        req_id: req_id.saturating_sub(1),
                        pos: active.play_pos.unwrap_or_default().as_millis() as _,
                        id: active.request.track.id,
                    };
                }
            }
            &PlayerState::Seeked { pos, id, req_id } => {
                // TODO this didn't update during a seek if we changed the state
                // if we're loading, then we should show a ghost or something
                // or just a spinner, then update to this (so we need an intermediate state)
                if let Some(Active { play_pos, .. }) = &mut self.active {
                    log::debug!(
                        "changing pos from: {play_pos:.2?} -> {pos:.2?}",
                        pos = Duration::from_millis(pos as _)
                    );
                    *play_pos.get_or_insert_with(Duration::default) =
                        Duration::from_millis(pos as _);
                }
                self.player_state = PlayerState::Playing { req_id, pos, id };
            }
            _ => {}
        }
    }
}

impl eframe::App for Control {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        static INITIALIZED: std::sync::Once = std::sync::Once::new();

        INITIALIZED.call_once(|| {
            frame.set_always_on_top(self.state.always_on_top);
        });

        ctx.request_repaint_after(Duration::from_secs_f32(1.0 / 30.0));

        if let Some(history) = self.history_fut.resolve() {
            self.history = history;
        }

        self.read_state();
        self.read_requests();
        self.read_events();

        self.cache.poll();

        if self.history_fut.is_resolved() {
            self.history.requests.reserve(self.out_of_band.len());

            for req in self.out_of_band.drain(..) {
                self.db.add_history(&req);
                self.history.requests.push(req)
            }
        }

        self.handle_key_presses(ctx, frame);

        CentralPanel::default().show(ctx, |ui| {
            // TODO use a projection type for this flow
            let mut replace = None;
            self.display_active(ui, &mut replace);
            self.check_state(&mut replace);
            self.handle_replace(replace);

            ui.separator();
            self.display_tab_list(ui);
        });
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.state.save(storage);
    }

    fn persist_egui_memory(&self) -> bool {
        false
    }
}

struct ControlState {
    volume: VolumeState,
    always_on_top: bool,
    auto_play: bool,
}

impl ControlState {
    const VOLUME_KEY: &str = concat!(env!("CARGO_PKG_NAME"), ".volume");
    const ALWAYS_ON_TOP_KEY: &str = concat!(env!("CARGO_PKG_NAME"), ".auto-play");
    const AUTO_PLAY_KEY: &str = concat!(env!("CARGO_PKG_NAME"), ".always-on-top");

    fn load(storage: &dyn eframe::Storage, volume: VolumeState) -> Self {
        fn get<T>(storage: &dyn eframe::Storage, key: &'static str) -> Option<T>
        where
            T: std::str::FromStr,
        {
            storage.get_string(key).and_then(|c| c.parse().ok())
        }

        if let Some(factor) = get(storage, Self::VOLUME_KEY) {
            volume.set(factor)
        }

        Self {
            volume,
            always_on_top: get(storage, Self::ALWAYS_ON_TOP_KEY).unwrap_or_default(),
            auto_play: get(storage, Self::AUTO_PLAY_KEY).unwrap_or_default(),
        }
    }

    fn save(&self, storage: &mut dyn eframe::Storage) {
        storage.set_string(Self::VOLUME_KEY, format!("{:.2}", self.volume.get()));
        storage.set_string(Self::ALWAYS_ON_TOP_KEY, self.auto_play.to_string());
        storage.set_string(Self::AUTO_PLAY_KEY, self.always_on_top.to_string());
    }
}
