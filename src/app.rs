use crate::engine::{Engine, EngineConfig, SearchReport};
use crate::persistent_cache::PersistentAnalysisCache;
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use kish::{Action, Board, Game, GameStatus, Square, Team};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, Sender},
    Arc,
};
use std::thread;
use std::time::Duration;

#[derive(Clone, Copy, PartialEq, Eq)]
enum PlayMode {
    HumanWhite,
    HumanBlack,
    TwoPlayers,
    WatchEngines,
}

impl PlayMode {
    fn label(self) -> &'static str {
        match self {
            Self::HumanWhite => "Play as White",
            Self::HumanBlack => "Play as Black",
            Self::TwoPlayers => "Two Players",
            Self::WatchEngines => "Engine vs Engine",
        }
    }

    fn human_controls(self, team: Team) -> bool {
        match self {
            Self::HumanWhite => team == Team::White,
            Self::HumanBlack => team == Team::Black,
            Self::TwoPlayers => true,
            Self::WatchEngines => false,
        }
    }

    fn is_human_engine(self) -> bool {
        matches!(self, Self::HumanWhite | Self::HumanBlack)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum JobPurpose {
    EngineMove,
    Analysis,
}

struct ActiveJob {
    id: u64,
    purpose: JobPurpose,
    cancel: Arc<AtomicBool>,
}

enum EngineMessage {
    Progress {
        id: u64,
        purpose: JobPurpose,
        report: SearchReport,
    },
    Finished {
        id: u64,
        purpose: JobPurpose,
        position: Board,
        report: Option<SearchReport>,
    },
}

struct MoveEntry {
    number: usize,
    team: Team,
    notation: String,
    by_engine: bool,
}

pub struct DraughtsApp {
    game: Game,
    mode: PlayMode,
    flipped: bool,
    selected: Option<Square>,
    pending_choices: Vec<Action>,
    last_move: Option<(Square, Square)>,
    move_log: Vec<MoveEntry>,

    move_time_secs: u64,
    analysis_time_secs: u64,
    max_depth: u32,
    analysis_enabled: bool,
    persistent_cache_enabled: bool,
    show_cache_hits: bool,
    analysis_cache: PersistentAnalysisCache,

    latest_report: Option<SearchReport>,
    latest_purpose: Option<JobPurpose>,
    analyzed_position: Option<Board>,
    status_message: String,

    tx: Sender<EngineMessage>,
    rx: Receiver<EngineMessage>,
    active_job: Option<ActiveJob>,
    next_job_id: u64,
}

impl DraughtsApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = Color32::from_rgb(17, 20, 27);
        visuals.window_fill = Color32::from_rgb(17, 20, 27);
        visuals.extreme_bg_color = Color32::from_rgb(12, 15, 21);
        visuals.widgets.active.bg_fill = Color32::from_rgb(52, 111, 105);
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(37, 57, 68);
        cc.egui_ctx.set_visuals(visuals);

        let mut style = (*cc.egui_ctx.global_style()).clone();
        style.spacing.item_spacing = Vec2::new(10.0, 8.0);
        style.spacing.button_padding = Vec2::new(12.0, 7.0);
        cc.egui_ctx.set_global_style(style);

        let (tx, rx) = mpsc::channel();

        Self {
            game: Game::new(),
            mode: PlayMode::HumanWhite,
            flipped: false,
            last_move: None,
            selected: None,
            pending_choices: Vec::new(),
            move_log: Vec::new(),
            move_time_secs: 3,
            analysis_time_secs: 2,
            max_depth: 14,
            analysis_enabled: true,
            persistent_cache_enabled: true,
            show_cache_hits: true,
            analysis_cache: PersistentAnalysisCache::load("analysis_cache.json"),
            latest_report: None,
            latest_purpose: None,
            analyzed_position: None,
            status_message: "Your move. Select a piece.".to_owned(),
            tx,
            rx,
            active_job: None,
            next_job_id: 0,
        }
    }

    fn new_game(&mut self) {
        self.cancel_active_job();
        self.game = Game::new();
        self.selected = None;
        self.pending_choices.clear();
        self.move_log.clear();
        self.latest_report = None;
        self.latest_purpose = None;
        self.analyzed_position = None;
        self.status_message = "New game started.".to_owned();
        self.last_move = None;
    }

    fn cancel_active_job(&mut self) {
        if let Some(job) = self.active_job.take() {
            job.cancel.store(true, Ordering::Relaxed);
        }
    }

    fn is_human_turn(&self) -> bool {
        self.mode.human_controls(self.game.turn())
    }

    fn game_in_progress(&self) -> bool {
        matches!(self.game.status(), GameStatus::InProgress)
    }

    fn spawn_search(&mut self, ctx: &egui::Context, purpose: JobPurpose) {
        if self.active_job.is_some() || !self.game_in_progress() {
            return;
        }

        let position = *self.game.board();
        if purpose == JobPurpose::Analysis && self.persistent_cache_enabled {
            let key = Engine::board_cache_key(position);
            if let Some(entry) = self.analysis_cache.lookup(&key, self.max_depth) {
                self.status_message = "Analysis loaded from persistent cache.".to_owned();
                let cached_report = SearchReport {
                    best_action: None,
                    score_white: entry.score_white,
                    completed_depth: entry.depth,
                    nodes: 0,
                    qnodes: 0,
                    tt_hits: 0,
                    cutoffs: 0,
                    elapsed: Duration::from_millis(0),
                    nps: 0,
                    tt_entries: 0,
                    principal_variation: entry.best_move.into_iter().collect(),
                    forced_root: false,
                };
                self.latest_report = Some(cached_report);
                self.latest_purpose = Some(JobPurpose::Analysis);
                self.analyzed_position = Some(position);
                return;
            }
        }
        self.next_job_id += 1;
        let id = self.next_job_id;
        let cancel = Arc::new(AtomicBool::new(false));
        let thread_cancel = cancel.clone();
        let tx = self.tx.clone();
        let repaint = ctx.clone();

        let seconds = if purpose == JobPurpose::EngineMove {
            self.move_time_secs
        } else {
            self.analysis_time_secs
        };
        let config = EngineConfig::play(self.max_depth, seconds);

        self.active_job = Some(ActiveJob {
            id,
            purpose,
            cancel,
        });

        self.status_message = match purpose {
            JobPurpose::EngineMove => "Engine is thinking...".to_owned(),
            JobPurpose::Analysis => "Analysing current position...".to_owned(),
        };

        thread::spawn(move || {
            let mut engine = Engine::new(config, thread_cancel, position);
            let result = engine.search(|report| {
                let _ = tx.send(EngineMessage::Progress {
                    id,
                    purpose,
                    report,
                });
                repaint.request_repaint();
            });

            let _ = tx.send(EngineMessage::Finished {
                id,
                purpose,
                position,
                report: result,
            });
            repaint.request_repaint();
        });
    }

    fn poll_engine_messages(&mut self) {
        while let Ok(message) = self.rx.try_recv() {
            match message {
                EngineMessage::Progress {
                    id,
                    purpose,
                    report,
                } => {
                    if self.active_job.as_ref().map(|job| job.id) == Some(id) {
                        self.latest_report = Some(report);
                        self.latest_purpose = Some(purpose);
                    }
                }
                EngineMessage::Finished {
                    id,
                    purpose,
                    position,
                    report,
                } => {
                    if self.active_job.as_ref().map(|job| job.id) != Some(id) {
                        continue;
                    }

                    self.active_job = None;
                    if let Some(report) = report {
                        self.latest_report = Some(report.clone());
                        self.latest_purpose = Some(purpose);

                        if purpose == JobPurpose::EngineMove
                            && *self.game.board() == position
                            && self.game_in_progress()
                        {
                            if let Some(action) = report.best_action {
                                self.apply_move(action, true);
                            }
                        } else if purpose == JobPurpose::Analysis {
                            self.analyzed_position = Some(position);
                            self.status_message = "Analysis completed.".to_owned();
                            if self.persistent_cache_enabled {
                                let key = Engine::board_cache_key(position);
                                self.analysis_cache.upsert_root(key, &report);
                                self.analysis_cache.flush_atomic();
                            }
                        }
                    } else {
                        self.status_message = "Search stopped before a depth completed.".to_owned();
                    }
                }
            }
        }
    }

    fn request_next_work(&mut self, ctx: &egui::Context) {
        if self.active_job.is_some() || !self.game_in_progress() {
            return;
        }

        if !self.is_human_turn() {
            self.spawn_search(ctx, JobPurpose::EngineMove);
        } else if self.analysis_enabled && self.analyzed_position != Some(*self.game.board()) {
            self.spawn_search(ctx, JobPurpose::Analysis);
        }
    }

    fn apply_move(&mut self, action: Action, by_engine: bool) {
        let board = *self.game.board();
        let notation = action.to_detailed(board.turn, &board.state).to_notation();
        let from = action.source(board.turn, board.friendly_pieces());
        let to = action.destination(board.turn, board.friendly_pieces());
        self.last_move = Some((from, to));
        let team = board.turn;
        let number = self.move_log.len() / 2 + 1;

        if !by_engine {
            self.cancel_active_job();
        }

        self.game.make_move(&action);
        self.move_log.push(MoveEntry {
            number,
            team,
            notation: notation.clone(),
            by_engine,
        });

        self.selected = None;
        self.pending_choices.clear();
        self.analyzed_position = None;
        self.status_message = if by_engine {
            format!("Engine played {notation}.")
        } else {
            format!("You played {notation}.")
        };

        match self.game.status() {
            GameStatus::InProgress => {}
            GameStatus::Draw => self.status_message = "Game over: draw.".to_owned(),
            GameStatus::Won(team) => {
                self.status_message = format!("Game over: {team} wins.");
            }
        }
    }

    fn undo(&mut self) {
        self.cancel_active_job();
        let plies = if self.mode.is_human_engine() || self.mode == PlayMode::WatchEngines {
            2
        } else {
            1
        };

        for _ in 0..plies {
            if self.game.undo_move() {
                self.move_log.pop();
            }
        }

        self.selected = None;
        self.pending_choices.clear();
        self.last_move = None;
        self.latest_report = None;
        self.latest_purpose = None;
        self.analyzed_position = None;
        self.status_message = "Move undone.".to_owned();
    }

    fn click_square(&mut self, square: Square) {
        if !self.game_in_progress() || !self.is_human_turn() {
            return;
        }
        if self
            .active_job
            .as_ref()
            .is_some_and(|job| job.purpose == JobPurpose::EngineMove)
        {
            return;
        }

        let board = *self.game.board();
        let legal = self.game.actions();

        if let Some(source) = self.selected {
            let matches: Vec<Action> = legal
                .iter()
                .copied()
                .filter(|action| {
                    action.source(board.turn, board.friendly_pieces()) == source
                        && action.destination(board.turn, board.friendly_pieces()) == square
                })
                .collect();

            if matches.len() == 1 {
                self.apply_move(matches[0], false);
                return;
            } else if matches.len() > 1 {
                self.pending_choices = matches;
                self.status_message =
                    "Multiple capture routes reach this square. Choose the route on the right."
                        .to_owned();
                return;
            }
        }

        let can_select = legal
            .iter()
            .any(|action| action.source(board.turn, board.friendly_pieces()) == square);

        if can_select {
            self.selected = Some(square);
            self.pending_choices.clear();
        } else {
            self.selected = None;
            self.pending_choices.clear();
        }
    }

    fn legal_targets(&self) -> Vec<Square> {
        let Some(source) = self.selected else {
            return Vec::new();
        };
        let board = *self.game.board();

        self.game
            .actions()
            .iter()
            .filter(|action| action.source(board.turn, board.friendly_pieces()) == source)
            .map(|action| action.destination(board.turn, board.friendly_pieces()))
            .collect()
    }

    fn legal_sources(&self) -> Vec<Square> {
        let board = *self.game.board();
        self.game
            .actions()
            .iter()
            .map(|action| action.source(board.turn, board.friendly_pieces()))
            .collect()
    }

    fn render_header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("KISH  ·  TURKISH DRAUGHTS STUDIO");
            ui.add_space(18.0);
            let turn = self.game.turn();
            ui.label(egui::RichText::new(format!("Turn: {turn}")).strong().color(
                if turn == Team::White {
                    Color32::from_rgb(231, 232, 235)
                } else {
                    Color32::from_rgb(168, 179, 194)
                },
            ));
            ui.separator();
            ui.label(&self.status_message);
        });
    }

    fn render_controls(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading("Game Controls");
        ui.add_space(4.0);

        let old_mode = self.mode;
        egui::ComboBox::from_label("Mode / your colour")
            .selected_text(self.mode.label())
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut self.mode,
                    PlayMode::HumanWhite,
                    PlayMode::HumanWhite.label(),
                );
                ui.selectable_value(
                    &mut self.mode,
                    PlayMode::HumanBlack,
                    PlayMode::HumanBlack.label(),
                );
                ui.selectable_value(
                    &mut self.mode,
                    PlayMode::TwoPlayers,
                    PlayMode::TwoPlayers.label(),
                );
                ui.selectable_value(
                    &mut self.mode,
                    PlayMode::WatchEngines,
                    PlayMode::WatchEngines.label(),
                );
            });

        if old_mode != self.mode {
            self.flipped = self.mode == PlayMode::HumanBlack;
            self.new_game();
        }

        ui.horizontal(|ui| {
            if ui.button("New Game").clicked() {
                self.new_game();
            }
            if ui.button("Undo Turn").clicked() {
                self.undo();
            }
            if ui.button("Flip Board").clicked() {
                self.flipped = !self.flipped;
            }
        });

        ui.separator();
        ui.heading("Engine Settings");
        ui.add(egui::Slider::new(&mut self.move_time_secs, 1..=15).text("Move time (s)"));
        ui.add(egui::Slider::new(&mut self.max_depth, 4..=24).text("Maximum depth"));
        ui.checkbox(&mut self.analysis_enabled, "Live analysis on your turn");
        ui.checkbox(
            &mut self.persistent_cache_enabled,
            "Enable persistent analysis cache",
        );
        ui.checkbox(&mut self.show_cache_hits, "Show cache hits");
        if ui.button("Clear cache").clicked() {
            self.analysis_cache.clear();
            self.status_message = "Persistent analysis cache cleared.".to_owned();
        }
        ui.add_enabled(
            self.analysis_enabled,
            egui::Slider::new(&mut self.analysis_time_secs, 1..=10).text("Analysis time (s)"),
        );

        ui.horizontal(|ui| {
            if ui
                .add_enabled(self.active_job.is_none(), egui::Button::new("Analyse Now"))
                .clicked()
            {
                self.analyzed_position = None;
                self.spawn_search(ctx, JobPurpose::Analysis);
            }

            if ui
                .add_enabled(self.active_job.is_some(), egui::Button::new("Stop"))
                .clicked()
            {
                self.cancel_active_job();
                self.status_message = "Search stopped.".to_owned();
            }
        });

        if self.active_job.is_some() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Searching...");
            });
        }
    }

    fn render_analysis(&self, ui: &mut egui::Ui) {
        ui.separator();
        ui.heading("Analysis");
        let static_eval = Engine::evaluate_white_static(*self.game.board());

        let eval = self
            .latest_report
            .as_ref()
            .map(|r| r.score_white)
            .unwrap_or(static_eval);

        ui.label(egui::RichText::new("Engine evaluation (White perspective)").small());
        self.render_eval_bar(ui, eval);

        if let Some(report) = &self.latest_report {
            let kind = match self.latest_purpose {
                Some(JobPurpose::EngineMove) => "Move search",
                Some(JobPurpose::Analysis) => "Position analysis",
                None => "Analysis",
            };

            ui.label(format!("{kind} · depth {}", report.completed_depth));
            ui.label(format!(
                "Nodes: {}  |  QNodes: {}",
                format_number(report.nodes),
                format_number(report.qnodes)
            ));
            ui.label(format!(
                "NPS: {}  |  Time: {:.3}s",
                format_number(report.nps),
                report.elapsed.as_secs_f64()
            ));
            ui.label(format!(
                "TT hits: {}  |  Entries: {}",
                format_number(report.tt_hits),
                format_number(report.tt_entries as u64)
            ));
            ui.label(format!("Cutoffs: {}", format_number(report.cutoffs)));
            if self.show_cache_hits {
                ui.label(format!(
                    "Persistent cache hits: {}",
                    format_number(self.analysis_cache.hits)
                ));
            }

            ui.add_space(5.0);
            ui.label(egui::RichText::new("Principal variation").strong());
            if report.principal_variation.is_empty() {
                ui.label("-");
            } else {
                ui.label(report.principal_variation.join("  "));
            }
        } else {
            ui.label("No completed engine analysis yet.");
        }

        ui.add_space(5.0);
        ui.label(
            egui::RichText::new(
                "Evaluation is the engine's best estimate, not a solved tablebase result.",
            )
            .small()
            .color(Color32::from_rgb(147, 158, 170)),
        );
    }

    fn render_eval_bar(&self, ui: &mut egui::Ui, score: i32) {
        let text = if score.abs() >= 900_000 {
            if score > 0 {
                "White winning".to_owned()
            } else {
                "Black winning".to_owned()
            }
        } else {
            format!("{:+.2}", score as f32 / 100.0)
        };

        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 24.0), Sense::hover());
        let painter = ui.painter();
        painter.rect_filled(rect, 6.0, Color32::from_rgb(36, 42, 53));

        let normalized = ((score as f32 / 600.0).clamp(-1.0, 1.0) + 1.0) / 2.0;
        let fill_rect = Rect::from_min_max(
            rect.min,
            Pos2::new(rect.left() + rect.width() * normalized, rect.bottom()),
        );
        painter.rect_filled(fill_rect, 6.0, Color32::from_rgb(218, 220, 222));
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            text,
            FontId::proportional(13.0),
            Color32::from_rgb(8, 11, 16),
        );
    }

    fn render_moves(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        ui.heading("Move History");

        if !self.pending_choices.is_empty() {
            ui.label(egui::RichText::new("Choose capture route:").strong());
            let board = *self.game.board();
            let choices = self.pending_choices.clone();
            for action in choices {
                let notation = action.to_detailed(board.turn, &board.state).to_notation();
                if ui.button(notation).clicked() {
                    self.apply_move(action, false);
                }
            }
            ui.separator();
        }

        egui::ScrollArea::vertical()
            .max_height(230.0)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                if self.move_log.is_empty() {
                    ui.label("No moves played.");
                } else {
                    for entry in &self.move_log {
                        let prefix = if entry.team == Team::White { "W" } else { "B" };
                        let actor = if entry.by_engine { "engine" } else { "player" };
                        ui.label(format!(
                            "{:>2}. {}  {:<10}  ({})",
                            entry.number, prefix, entry.notation, actor
                        ));
                    }
                }
            });
    }

    fn render_board(&mut self, ui: &mut egui::Ui) {
        let board = *self.game.board();
        let available = ui.available_size();
        let size = available.x.min(available.y).min(720.0).max(420.0);
        let cell = size / 8.0;
        let (board_rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());

        let painter = ui.painter();
        let legal_sources = self.legal_sources();
        let targets = self.legal_targets();

        for screen_row in 0..8usize {
            for screen_col in 0..8usize {
                let (row, col) = if self.flipped {
                    (screen_row, 7 - screen_col)
                } else {
                    (7 - screen_row, screen_col)
                };
                let index = row * 8 + col;
                let square = Square::try_from_usize(index).expect("board index is always valid");
                let min = Pos2::new(
                    board_rect.left() + screen_col as f32 * cell,
                    board_rect.top() + screen_row as f32 * cell,
                );
                let rect = Rect::from_min_size(min, Vec2::splat(cell));

                let light = (row + col) % 2 == 0;
                let base = if light {
                    Color32::from_rgb(205, 183, 151)
                } else {
                    Color32::from_rgb(91, 70, 58)
                };

                painter.rect_filled(rect, 0.0, base);

                if legal_sources.contains(&square)
                    && self.selected.is_none()
                    && self.is_human_turn()
                {
                    painter.circle_filled(
                        rect.center(),
                        cell * 0.07,
                        Color32::from_rgba_premultiplied(45, 120, 112, 150),
                    );
                }

                if self.selected == Some(square) {
                    painter.rect_filled(
                        rect.shrink(cell * 0.03),
                        4.0,
                        Color32::from_rgba_premultiplied(56, 166, 153, 90),
                    );
                }

                if targets.contains(&square) {
                    painter.circle_filled(
                        rect.center(),
                        cell * 0.12,
                        Color32::from_rgba_premultiplied(60, 185, 166, 175),
                    );
                }
                if let Some((from, to)) = self.last_move {
                    if square == from {
                        painter.rect_filled(
                            rect.shrink(cell * 0.04),
                            6.0,
                            Color32::from_rgba_premultiplied(255, 210, 90, 95),
                        );
                    }

                    if square == to {
                        painter.rect_filled(
                            rect.shrink(cell * 0.04),
                            6.0,
                            Color32::from_rgba_premultiplied(80, 210, 160, 110),
                        );
                    }
                }
                let mask = square.to_mask();
                let white_piece = board.state.pieces[0] & mask != 0;
                let black_piece = board.state.pieces[1] & mask != 0;
                let king = board.state.kings & mask != 0;

                if white_piece || black_piece {
                    let center = rect.center();
                    let fill = if white_piece {
                        Color32::from_rgb(239, 237, 230)
                    } else {
                        Color32::from_rgb(27, 31, 38)
                    };
                    let border = if white_piece {
                        Color32::from_rgb(128, 120, 108)
                    } else {
                        Color32::from_rgb(195, 158, 96)
                    };
                    painter.circle_filled(center, cell * 0.36, fill);
                    painter.circle_stroke(center, cell * 0.36, Stroke::new(2.0, border));
                    painter.circle_stroke(center, cell * 0.29, Stroke::new(1.0, border));

                    if king {
                        painter.text(
                            center,
                            egui::Align2::CENTER_CENTER,
                            "K",
                            FontId::proportional(cell * 0.34),
                            border,
                        );
                    }
                }

                let response = ui.interact(
                    rect,
                    ui.make_persistent_id(("square", square.to_usize())),
                    Sense::click(),
                );
                if response.clicked() {
                    self.click_square(square);
                }
            }
        }

        for i in 0..8usize {
            let file_col = if self.flipped { 7 - i } else { i };
            let rank_row = if self.flipped { i } else { 7 - i };
            let file = (b'a' + file_col as u8) as char;
            let rank = rank_row + 1;

            painter.text(
                Pos2::new(
                    board_rect.left() + i as f32 * cell + 6.0,
                    board_rect.bottom() - 6.0,
                ),
                egui::Align2::LEFT_BOTTOM,
                file.to_string(),
                FontId::proportional(12.0),
                Color32::from_rgb(238, 225, 201),
            );
            painter.text(
                Pos2::new(
                    board_rect.left() + 6.0,
                    board_rect.top() + i as f32 * cell + 6.0,
                ),
                egui::Align2::LEFT_TOP,
                rank.to_string(),
                FontId::proportional(12.0),
                Color32::from_rgb(238, 225, 201),
            );
        }
    }
}

impl eframe::App for DraughtsApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_engine_messages();
        self.request_next_work(ui.ctx());

        ui.add_space(8.0);
        self.render_header(ui);
        ui.add_space(12.0);
        ui.separator();
        ui.add_space(10.0);

        ui.columns(2, |columns| {
            columns[0].set_min_width(640.0);
            self.render_board(&mut columns[0]);

            let side_ctx = columns[1].ctx().clone();

            columns[1].vertical(|ui| {
                self.render_controls(ui, &side_ctx);
                self.render_analysis(ui);
                self.render_moves(ui);
            });
        });

        if self.active_job.is_some() {
            ui.ctx().request_repaint_after(Duration::from_millis(80));
        }
    }
}

fn format_number(value: u64) -> String {
    let raw = value.to_string();
    let mut out = String::new();
    let chars: Vec<char> = raw.chars().collect();
    for (i, ch) in chars.iter().enumerate() {
        if i > 0 && (chars.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(*ch);
    }
    out
}
