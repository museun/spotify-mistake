use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use egui::{Align, CentralPanel, FontDefinitions, FontTweak, Layout};

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
    ext::JoinWith,
    history::{self, History, HistoryItem},
    history_view::HistoryView,
    image_cache::ImageCache,
    list_view::ListView,
    player_state::{NextPlayingState, PlayerState},
    queue_view::QueueView,
    request::Request,
    tab_view::TabView,
    volume_state::VolumeState,
};

use self::{active_control::ActiveControl, info_panel::InfoPanel};

mod active_control;
mod info_panel;
mod lyrics_panel;
mod player_control;

struct Active {
    // TODO we need an FSM for resuming from a pause state
    start: Option<Instant>,
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
    volume: VolumeState,

    player: Player,
    player_state: PlayerState,
    player_events: PlayerEventChannel,
    next_playing: NextPlayingState,

    always_on_top: bool,
    auto_play: bool,
    tab_view: TabView,
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

        Self::load_fonts(&cc.egui_ctx);

        let history_fut = std::fs::read_to_string("history.json")
            .ok()
            .map(|s| History::load(&s, &session))
            .unwrap_or_default();

        if let Some(storage) = cc.storage {
            Self::load_saved_state(storage, &session, replay);
            if let Some(factor) = storage
                .get_string("volume")
                .and_then(|c| c.parse::<f64>().ok())
            {
                volume.set(factor);
            }
        }

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
            volume,
            player_state: PlayerState::default(),

            next_playing: NextPlayingState::default(),

            always_on_top: false,
            auto_play: false,
            tab_view: TabView::default(),
        })
    }

    fn load_saved_state(
        storage: &dyn eframe::Storage,
        session: &Session,
        replay: UnboundedSender<SynthEvent<Request>>,
    ) {
        #[derive(::serde::Deserialize)]
        struct State {
            active: Option<HistoryItem<'static>>,
            queue: Vec<HistoryItem<'static>>,
        }

        let send_events = move |items: Vec<Request>| {
            items
                .into_iter()
                .map(SynthEvent::Synthetic)
                .try_for_each(|item| replay.send(item))
                .ok()
                .expect("control state incontinuity");
        };

        if let Some(state) = storage
            .get_string("saved_tracks")
            .and_then(|s| serde_json::from_str::<State>(&s).ok())
        {
            history::lookup_all_requests(
                state.active.into_iter().chain(state.queue),
                session.clone(),
                send_events,
            );
        }
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
            for tab_view in [TabView::Queue, TabView::History] {
                ui.selectable_value(&mut self.tab_view, tab_view, tab_view.label());
            }
        });

        let list_view = ListView {
            cache: &mut self.cache,
        };

        match self.tab_view {
            TabView::Queue => {
                QueueView {
                    list_view,
                    queue: &mut self.queue,
                }
                .display(ui);
            }
            TabView::History => {
                HistoryView {
                    list_view,
                    queue: &mut self.queue,
                    history: &mut self.history,
                }
                .display(ui);
            }
        }
    }

    fn read_events(&mut self) {
        while let Ok(req) = self.events.try_recv() {
            let req = match req {
                SynthEvent::Synthetic(req) => req,
                // TODO check for duplicates
                SynthEvent::Organic(req) => {
                    let place = if self.history_fut.is_resolved() {
                        &mut self.history.requests
                    } else {
                        &mut self.out_of_band
                    };
                    place.push(req.clone());
                    req
                }
            };

            self.player.preload(req.track.id);

            if self.active.is_none() {
                self.active.replace(Active {
                    start: None,
                    request: req,
                });
                continue;
            }
            self.queue.push_back(req);
        }
    }

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
            self.always_on_top = !self.always_on_top;
            frame.set_always_on_top(self.always_on_top);
        }
    }

    fn display_active(&mut self, ui: &mut egui::Ui, replace: &mut Option<Request>) {
        ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
            let Some(Active { request, start }) = &mut self.active else {
                return
            };

            let elapsed = start.map(|s| s.elapsed().as_millis() as usize);

            let resp = ActiveControl {
                request,
                start,
                elapsed,
                cache: &mut self.cache,
                queue: &mut self.queue,
                auto_play: &mut self.auto_play,
                player: &mut self.player,
                player_state: &self.player_state,
                volume: &self.volume,
            }
            .display(ui, replace);

            InfoPanel {
                request,
                cache: &mut self.cache,
                elapsed,
                height: resp.rect.height(),
            }
            .display(ui);
        });
    }

    fn handle_replace(&mut self, replace: Option<Request>) {
        let Some(request) = replace else { return };

        let _ = self.active.replace(Active {
            start: None,
            request,
        });
        self.player.stop();
        let _ = std::mem::take(&mut self.next_playing);

        if !self.auto_play {
            return;
        }

        let Some(Active { request, .. }) = &self.active else { return };

        log::info!(
            "auto-playing: {name} by {artist} requested by: {user} ({user_id})",
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
            PlayerState::Playing { req_id, id, .. } => {
                self.next_playing = NextPlayingState::Playing;
            }
            PlayerState::EndOfPlaying { req_id, id }
                if matches!(self.next_playing, NextPlayingState::Playing) && self.auto_play =>
            {
                *replace = self.queue.pop_front();
            }
            _ => {}
        }
    }
}

impl eframe::App for Control {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        ctx.request_repaint_after(Duration::from_secs_f32(1.0 / 30.0));

        if let Some(history) = self.history_fut.resolve() {
            self.history = history;
        }

        self.read_state();
        self.read_requests();
        self.read_events();

        self.cache.poll();

        if self.history_fut.is_resolved() {
            self.history.requests.append(&mut self.out_of_band)
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
        #[derive(::serde::Serialize)]
        struct State<'a> {
            active: Option<history::HistoryItem<'a>>,
            queue: Vec<history::HistoryItem<'a>>,
        }

        storage.set_string("saved_tracks", {
            serde_json::to_string(&State {
                active: self
                    .active
                    .as_ref()
                    .map(|Active { request, .. }| history::HistoryItem::map(request)),
                queue: self.queue.iter().map(history::HistoryItem::map).collect(),
            })
            .unwrap()
        });

        let _ = std::fs::write("history.json", self.history.save());
        storage.set_string("volume", format!("{:.2}", self.volume.get()))
    }

    fn persist_egui_memory(&self) -> bool {
        false
    }
}
