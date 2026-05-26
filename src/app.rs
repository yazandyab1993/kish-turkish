use crate::engine::{Engine, EngineConfig, SearchReport};
use crate::opening_book::{self, OpeningBook, Side};
use crate::variation_book::{self, VariationBook};
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use kish::{Action, Board, Game, GameStatus, Square, Team};
use std::collections::HashMap;
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum SidePanelTab {
    Controls,
    Analysis,
    Moves,
}

impl SidePanelTab {
    fn label(self) -> &'static str {
        match self {
            Self::Controls => "Controls",
            Self::Analysis => "Analysis",
            Self::Moves => "Moves",
        }
    }
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

#[derive(Debug)]
enum BookLoadError {
    NotFound,
    Read(std::io::Error),
    Parse(serde_json::Error),
}

impl std::fmt::Display for BookLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "opening_book.json not found"),
            Self::Read(err) => write!(f, "failed to read opening book: {err}"),
            Self::Parse(err) => write!(f, "invalid opening book JSON: {err}"),
        }
    }
}

#[derive(Default)]
struct Diagnostics {
    variation_hits: u64,
    variation_misses: u64,
    book_hits: u64,
    book_misses: u64,
    book_skipped_due_to_variation_hit: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BookPriorityMode {
    VariationFirst,
    OpeningFirst,
    VariationOnly,
    OpeningOnly,
}

impl BookPriorityMode {
    fn label(self) -> &'static str {
        match self {
            Self::VariationFirst => "Variation first",
            Self::OpeningFirst => "Opening first",
            Self::VariationOnly => "Variation only",
            Self::OpeningOnly => "Opening only",
        }
    }
}

#[derive(Clone)]
struct AnalysisSnapshot {
    depth: u32,
    nodes: u64,
    score_white: i32,
    elapsed_secs: f64,
    pv: Vec<String>,
}

struct MoveEntry {
    number: usize,
    team: Team,
    notation: String,
    by_engine: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EditTool {
    Move,
    AddWhiteMan,
    AddBlackMan,
    AddWhiteKing,
    AddBlackKing,
    Remove,
    ToggleKing,
}

impl EditTool {
    fn label(self) -> &'static str {
        match self {
            Self::Move => "Move",
            Self::AddWhiteMan => "+ White",
            Self::AddBlackMan => "+ Black",
            Self::AddWhiteKing => "+ White King",
            Self::AddBlackKing => "+ Black King",
            Self::Remove => "Remove",
            Self::ToggleKing => "Toggle King",
        }
    }
}

pub struct DraughtsApp {
    game: Game,
    mode: PlayMode,
    flipped: bool,
    selected: Option<Square>,
    pending_choices: Vec<Action>,
    last_move: Option<(Square, Square)>,
    move_log: Vec<MoveEntry>,

    edit_mode: bool,
    edit_board: Board,
    edit_selected: Option<Square>,
    edit_tool: EditTool,
    edit_undo_stack: Vec<Board>,
    edit_redo_stack: Vec<Board>,

    move_time_secs: u64,
    analysis_time_secs: u64,
    max_depth: u32,
    max_nodes_millions: u64,
    use_time_limit: bool,
    use_depth_limit: bool,
    use_nodes_limit: bool,
    analysis_enabled: bool,
    analysis_continuous: bool,
    analysis_depth_step: u32,
    analysis_paused_by_user: bool,
    analysis_use_time_limit: bool,
    analysis_use_depth_limit: bool,
    analysis_use_nodes_limit: bool,
    analysis_max_depth: u32,
    analysis_max_nodes_millions: u64,
    opening_book_enabled: bool,
    opening_book: Option<OpeningBook>,
    variation_book_enabled: bool,
    variation_book_path: String,
    variation_book: Option<VariationBook>,
    book_priority_mode: BookPriorityMode,

    latest_report: Option<SearchReport>,
    latest_purpose: Option<JobPurpose>,
    analyzed_position: Option<Board>,
    status_message: String,

    tx: Sender<EngineMessage>,
    rx: Receiver<EngineMessage>,
    active_job: Option<ActiveJob>,
    next_job_id: u64,
    diagnostics: Diagnostics,
    analysis_cache: HashMap<String, SearchReport>,
    analysis_variations: Vec<AnalysisSnapshot>,
    side_panel_tab: SidePanelTab,
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

        let (opening_book, status_message) = match Self::load_opening_book() {
            Ok(book) => (Some(book), "Your move. Select a piece.".to_owned()),
            Err(err) => (None, format!("Your move. Select a piece. ({err})")),
        };
        let variation_book_path = "variations.txt".to_owned();
        let (variation_book, variation_status) =
            Self::load_variation_book(std::path::Path::new(&variation_book_path));
        let status_message = if variation_status.is_empty() {
            status_message
        } else {
            format!("{status_message} ({variation_status})")
        };

        Self {
            game: Game::new(),
            mode: PlayMode::HumanWhite,
            flipped: false,
            last_move: None,
            selected: None,
            pending_choices: Vec::new(),
            move_log: Vec::new(),
            edit_mode: false,
            edit_board: *Game::new().board(),
            edit_selected: None,
            edit_tool: EditTool::Move,
            edit_undo_stack: Vec::new(),
            edit_redo_stack: Vec::new(),
            move_time_secs: 3,
            analysis_time_secs: 2,
            max_depth: 14,
            max_nodes_millions: 10,
            use_time_limit: true,
            use_depth_limit: true,
            use_nodes_limit: false,
            analysis_enabled: true,
            analysis_continuous: false,
            analysis_depth_step: 2,
            analysis_paused_by_user: false,
            analysis_use_time_limit: true,
            analysis_use_depth_limit: true,
            analysis_use_nodes_limit: false,
            analysis_max_depth: 16,
            analysis_max_nodes_millions: 20,
            opening_book_enabled: true,
            opening_book,
            variation_book_enabled: true,
            variation_book_path,
            variation_book,
            book_priority_mode: BookPriorityMode::VariationFirst,
            latest_report: None,
            latest_purpose: None,
            analyzed_position: None,
            status_message,
            tx,
            rx,
            active_job: None,
            next_job_id: 0,
            diagnostics: Diagnostics::default(),
            analysis_cache: HashMap::new(),
            analysis_variations: Vec::new(),
            side_panel_tab: SidePanelTab::Controls,
        }
    }

    fn load_opening_book() -> Result<OpeningBook, BookLoadError> {
        let path = std::path::Path::new("opening_book.json");
        if !path.exists() {
            return Err(BookLoadError::NotFound);
        }

        let content = std::fs::read_to_string(path).map_err(BookLoadError::Read)?;
        serde_json::from_str::<OpeningBook>(&content).map_err(BookLoadError::Parse)
    }

    fn load_variation_book(path: &std::path::Path) -> (Option<VariationBook>, String) {
        if !path.exists() {
            return (None, "variation book file not found".to_owned());
        }
        match variation_book::load_variation_book(path) {
            Ok((book, warnings)) => {
                let mut status = format!(
                    "variation lines: {} loaded, {} rejected | positions indexed: {} | duplicates ignored: {}",
                    book.metadata.loaded_lines,
                    book.metadata.rejected_lines,
                    book.metadata.positions_indexed,
                    book.metadata.duplicate_positions_ignored
                );
                if let Some(first_warning) = warnings.first() {
                    status.push_str(&format!(" ({first_warning})"));
                }
                (Some(book), status)
            }
            Err(err) => (None, err.to_string()),
        }
    }

    fn try_opening_book_action(&self, board: Board) -> Option<Action> {
        if !self.opening_book_enabled {
            return None;
        }
        let book = self.opening_book.as_ref()?;
        let side = if board.turn == Team::White {
            Side::White
        } else {
            Side::Black
        };
        let raw = format!("{}", board.state);
        let rows: Vec<&str> = raw.lines().filter(|l| !l.trim().is_empty()).collect();
        if rows.len() != 8 {
            return None;
        }

        let mut converted = [['-'; 8]; 8];
        for (r, row) in rows.iter().enumerate() {
            let chars: Vec<char> = row.chars().filter(|ch| !ch.is_whitespace()).collect();
            if chars.len() != 8 {
                return None;
            }
            for (c, ch) in chars.into_iter().enumerate() {
                converted[r][c] = match ch {
                    'w' | 'W' | 'b' | 'B' | '-' => ch,
                    _ => return None,
                };
            }
        }

        let rec = opening_book::get_book_move(book, &converted, side, "best")?;
        let source = Self::square_from_name(&rec.from)?;
        let target = Self::square_from_name(&rec.to)?;

        board.actions().into_iter().find(|action| {
            action.source(board.turn, board.friendly_pieces()) == source
                && action.destination(board.turn, board.friendly_pieces()) == target
        })
    }

    fn try_variation_book_action(&self, board: Board) -> Option<Action> {
        if !self.variation_book_enabled {
            return None;
        }
        let book = self.variation_book.as_ref()?;
        let mv = variation_book::lookup_action(book, board)?;
        let source = Self::square_from_name(&mv.from)?;
        let target = Self::square_from_name(&mv.to)?;

        board.actions().into_iter().find(|action| {
            action.source(board.turn, board.friendly_pieces()) == source
                && action.destination(board.turn, board.friendly_pieces()) == target
        })
    }

    fn square_from_name(name: &str) -> Option<Square> {
        let mut chars = name.chars();
        let file = chars.next()?;
        let rank_ch = chars.next()?;
        if chars.next().is_some() {
            return None;
        }
        let file_idx = (file as u8).checked_sub(b'a')? as usize;
        let rank = rank_ch.to_digit(10)? as usize;
        if file_idx >= 8 || !(1..=8).contains(&rank) {
            return None;
        }
        let row = 8 - rank;
        let index = row * 8 + file_idx;
        Square::try_from_usize(index)
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
        self.analysis_paused_by_user = false;
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
        self.next_job_id += 1;
        let id = self.next_job_id;
        let cancel = Arc::new(AtomicBool::new(false));
        let thread_cancel = cancel.clone();
        let tx = self.tx.clone();
        let repaint = ctx.clone();

        let (
            seconds,
            use_time_limit,
            use_depth_limit,
            use_nodes_limit,
            target_depth,
            target_nodes_m,
        ) = if purpose == JobPurpose::EngineMove {
            (
                self.move_time_secs,
                self.use_time_limit,
                self.use_depth_limit,
                self.use_nodes_limit,
                self.max_depth,
                self.max_nodes_millions,
            )
        } else {
            (
                self.analysis_time_secs,
                self.analysis_use_time_limit,
                self.analysis_use_depth_limit,
                self.analysis_use_nodes_limit,
                self.analysis_max_depth,
                self.analysis_max_nodes_millions,
            )
        };
        let max_time = use_time_limit.then_some(Duration::from_secs(seconds));
        let max_depth = if purpose == JobPurpose::Analysis && self.analysis_continuous {
            Some(target_depth)
        } else {
            use_depth_limit.then_some(target_depth)
        };
        let max_nodes = use_nodes_limit.then_some(target_nodes_m.saturating_mul(1_000_000));
        let config = EngineConfig::with_limits(max_depth, max_time, max_nodes);

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

    fn poll_engine_messages(&mut self, ctx: &egui::Context) {
        while let Ok(message) = self.rx.try_recv() {
            match message {
                EngineMessage::Progress {
                    id,
                    purpose,
                    report,
                } => {
                    if self.active_job.as_ref().map(|job| job.id) == Some(id) {
                        self.latest_report = Some(report.clone());
                        self.latest_purpose = Some(purpose);
                        if purpose == JobPurpose::Analysis {
                            self.record_analysis_snapshot(&report);
                        }
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
                            let cache_key = Engine::board_cache_key(position);
                            self.analysis_cache.insert(cache_key, report.clone());

                            if self.analysis_continuous
                                && !self.analysis_paused_by_user
                                && self.game_in_progress()
                                && *self.game.board() == position
                            {
                                let next_depth = report
                                    .completed_depth
                                    .saturating_add(self.analysis_depth_step);
                                self.analysis_max_depth =
                                    self.analysis_max_depth.max(next_depth.min(60));
                                self.status_message = format!(
                                    "Depth {} reached. Continuing to depth {}...",
                                    report.completed_depth, self.analysis_max_depth
                                );
                                self.spawn_search(ctx, JobPurpose::Analysis);
                            } else {
                                self.status_message = "Analysis completed.".to_owned();
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
            let board = *self.game.board();
            let variation_action = self.try_variation_book_action(board);
            let opening_action = self.try_opening_book_action(board);

            let selected_action = match self.book_priority_mode {
                BookPriorityMode::VariationFirst => {
                    if let Some(action) = variation_action {
                        self.diagnostics.variation_hits += 1;
                        self.diagnostics.book_skipped_due_to_variation_hit += 1;
                        self.status_message =
                            "Engine played from variation book (mode: variation first).".to_owned();
                        Some(action)
                    } else {
                        self.diagnostics.variation_misses += 1;
                        if let Some(action) = opening_action {
                            self.diagnostics.book_hits += 1;
                            self.status_message =
                                "Engine played from opening book (mode: variation first)."
                                    .to_owned();
                            Some(action)
                        } else {
                            self.diagnostics.book_misses += 1;
                            None
                        }
                    }
                }
                BookPriorityMode::OpeningFirst => {
                    if let Some(action) = opening_action {
                        self.diagnostics.book_hits += 1;
                        self.status_message =
                            "Engine played from opening book (mode: opening first).".to_owned();
                        Some(action)
                    } else {
                        self.diagnostics.book_misses += 1;
                        if let Some(action) = variation_action {
                            self.diagnostics.variation_hits += 1;
                            self.status_message =
                                "Engine played from variation book (mode: opening first)."
                                    .to_owned();
                            Some(action)
                        } else {
                            self.diagnostics.variation_misses += 1;
                            None
                        }
                    }
                }
                BookPriorityMode::VariationOnly => {
                    if let Some(action) = variation_action {
                        self.diagnostics.variation_hits += 1;
                        self.status_message =
                            "Engine played from variation book (mode: variation only).".to_owned();
                        Some(action)
                    } else {
                        self.diagnostics.variation_misses += 1;
                        None
                    }
                }
                BookPriorityMode::OpeningOnly => {
                    if let Some(action) = opening_action {
                        self.diagnostics.book_hits += 1;
                        self.status_message =
                            "Engine played from opening book (mode: opening only).".to_owned();
                        Some(action)
                    } else {
                        self.diagnostics.book_misses += 1;
                        None
                    }
                }
            };

            if let Some(action) = selected_action {
                self.apply_move(action, true);
            } else {
                self.spawn_search(ctx, JobPurpose::EngineMove);
            }
        } else if self.analysis_enabled && self.analyzed_position != Some(*self.game.board()) {
            let cache_key = Engine::board_cache_key(*self.game.board());
            if let Some(cached) = self.analysis_cache.get(&cache_key) {
                self.latest_report = Some(cached.clone());
                self.latest_purpose = Some(JobPurpose::Analysis);
                self.analyzed_position = Some(*self.game.board());
                self.status_message = format!(
                    "Loaded cached analysis at depth {}.",
                    cached.completed_depth
                );
            } else {
                self.spawn_search(ctx, JobPurpose::Analysis);
            }
        }
    }

    fn record_analysis_snapshot(&mut self, report: &SearchReport) {
        if report.principal_variation.is_empty() {
            return;
        }
        let exists = self
            .analysis_variations
            .iter()
            .any(|v| v.depth == report.completed_depth && v.pv == report.principal_variation);
        if exists {
            return;
        }
        self.analysis_variations.push(AnalysisSnapshot {
            depth: report.completed_depth,
            nodes: report.nodes,
            score_white: report.score_white,
            elapsed_secs: report.elapsed.as_secs_f64(),
            pv: report.principal_variation.clone(),
        });
    }

    fn apply_move(&mut self, action: Action, by_engine: bool) {
        let board = if self.edit_mode {
            self.edit_board
        } else {
            *self.game.board()
        };
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
        self.analysis_paused_by_user = false;
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
        self.analysis_paused_by_user = false;
        self.status_message = "Move undone.".to_owned();
    }

    fn click_square(&mut self, square: Square) {
        if self.edit_mode || !self.game_in_progress() || !self.is_human_turn() {
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
        egui::Frame::new()
            .fill(Color32::from_rgb(20, 25, 33))
            .corner_radius(8.0)
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.heading("KISH  ·  TURKISH DRAUGHTS STUDIO");
                    ui.add_space(12.0);
                    let turn = self.game.turn();
                    ui.label(egui::RichText::new(format!("Turn: {turn}")).strong().color(
                        if turn == Team::White {
                            Color32::from_rgb(231, 232, 235)
                        } else {
                            Color32::from_rgb(168, 179, 194)
                        },
                    ));
                    ui.separator();
                    ui.label(egui::RichText::new(&self.status_message).small());
                });
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
            if ui.button("Edit Board").clicked() {
                self.cancel_active_job();
                self.edit_mode = true;
                self.edit_board = *self.game.board();
                self.edit_selected = None;
                self.edit_tool = EditTool::Move;
                self.edit_undo_stack.clear();
                self.edit_redo_stack.clear();
                self.status_message =
                    "Edit mode enabled. Use tools to move/add/remove pieces.".to_owned();
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
        ui.checkbox(&mut self.use_time_limit, "Stop at time limit");
        ui.add_enabled(
            self.use_time_limit,
            egui::Slider::new(&mut self.move_time_secs, 1..=30).text("Move time (s)"),
        );
        ui.checkbox(&mut self.use_depth_limit, "Stop at depth limit");
        ui.add_enabled(
            self.use_depth_limit,
            egui::Slider::new(&mut self.max_depth, 4..=30).text("Maximum depth"),
        );
        ui.checkbox(&mut self.use_nodes_limit, "Stop at nodes limit");
        ui.add_enabled(
            self.use_nodes_limit,
            egui::Slider::new(&mut self.max_nodes_millions, 1..=500).text("Nodes (million)"),
        );
        ui.checkbox(&mut self.analysis_enabled, "Live analysis");
        ui.separator();
        ui.heading("Analysis Session Settings");
        ui.checkbox(
            &mut self.analysis_use_time_limit,
            "Analysis: stop at time limit",
        );
        ui.add_enabled(
            self.analysis_use_time_limit,
            egui::Slider::new(&mut self.analysis_time_secs, 1..=30).text("Analysis time (s)"),
        );
        ui.checkbox(
            &mut self.analysis_use_depth_limit,
            "Analysis: stop at depth limit",
        );
        ui.add_enabled(
            self.analysis_use_depth_limit,
            egui::Slider::new(&mut self.analysis_max_depth, 4..=60).text("Analysis max depth"),
        );
        ui.checkbox(
            &mut self.analysis_use_nodes_limit,
            "Analysis: stop at nodes limit",
        );
        ui.add_enabled(
            self.analysis_use_nodes_limit,
            egui::Slider::new(&mut self.analysis_max_nodes_millions, 1..=500)
                .text("Analysis nodes (million)"),
        );
        ui.checkbox(&mut self.analysis_continuous, "Continuous depth climbing");
        ui.add_enabled(
            self.analysis_continuous,
            egui::Slider::new(&mut self.analysis_depth_step, 1..=8).text("Depth step"),
        );
        ui.checkbox(
            &mut self.opening_book_enabled,
            "Use opening book for engine moves",
        );
        ui.checkbox(
            &mut self.variation_book_enabled,
            "Use variation book for engine moves",
        );
        egui::ComboBox::from_label("Book priority mode")
            .selected_text(self.book_priority_mode.label())
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut self.book_priority_mode,
                    BookPriorityMode::VariationFirst,
                    BookPriorityMode::VariationFirst.label(),
                );
                ui.selectable_value(
                    &mut self.book_priority_mode,
                    BookPriorityMode::OpeningFirst,
                    BookPriorityMode::OpeningFirst.label(),
                );
                ui.selectable_value(
                    &mut self.book_priority_mode,
                    BookPriorityMode::VariationOnly,
                    BookPriorityMode::VariationOnly.label(),
                );
                ui.selectable_value(
                    &mut self.book_priority_mode,
                    BookPriorityMode::OpeningOnly,
                    BookPriorityMode::OpeningOnly.label(),
                );
            });
        ui.horizontal(|ui| {
            ui.label("Variation file:");
            ui.text_edit_singleline(&mut self.variation_book_path);
            if ui.button("Reload Variations").clicked() {
                let (book, status) =
                    Self::load_variation_book(std::path::Path::new(&self.variation_book_path));
                self.variation_book = book;
                self.status_message = format!("Variations reloaded: {status}");
            }
        });

        ui.horizontal(|ui| {
            let has_any_limit = self.analysis_use_time_limit
                || self.analysis_use_depth_limit
                || self.analysis_use_nodes_limit;
            if ui
                .add_enabled(
                    self.active_job.is_none() && has_any_limit,
                    egui::Button::new("Analyse Now"),
                )
                .clicked()
            {
                self.analyzed_position = None;
                self.analysis_paused_by_user = false;
                self.analysis_variations.clear();
                self.spawn_search(ctx, JobPurpose::Analysis);
            }

            if ui
                .add_enabled(self.active_job.is_some(), egui::Button::new("Pause"))
                .clicked()
            {
                self.cancel_active_job();
                self.analysis_paused_by_user = true;
                self.status_message = "Analysis paused by user.".to_owned();
            }
        });

        if self.active_job.is_some() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Searching...");
            });
        } else if !self.analysis_use_time_limit
            && !self.analysis_use_depth_limit
            && !self.analysis_use_nodes_limit
        {
            ui.colored_label(
                Color32::YELLOW,
                "Enable at least one analysis search limit.",
            );
        }
    }

    fn render_analysis(&mut self, ui: &mut egui::Ui) {
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
            ui.label(format!(
                "Variation hits: {}  |  Variation misses: {}",
                format_number(self.diagnostics.variation_hits),
                format_number(self.diagnostics.variation_misses)
            ));
            ui.label(format!(
                "Book hits: {}  |  Book misses: {}",
                format_number(self.diagnostics.book_hits),
                format_number(self.diagnostics.book_misses)
            ));
            ui.label(format!(
                "Book skipped (variation hit): {}",
                format_number(self.diagnostics.book_skipped_due_to_variation_hit)
            ));
            ui.add_space(5.0);
            ui.label(egui::RichText::new("Principal variation").strong());
            if report.principal_variation.is_empty() {
                ui.label("-");
            } else {
                ui.label(report.principal_variation.join("  "));
            }
            ui.label(format!(
                "PV length: {} plies",
                report.principal_variation.len()
            ));

            ui.add_space(6.0);
            ui.label(egui::RichText::new("Saved variations").strong());
            if self.analysis_variations.is_empty() {
                ui.label("No saved variation yet.");
            } else {
                egui::ScrollArea::vertical()
                    .max_height(220.0)
                    .show(ui, |ui| {
                        for snapshot in &self.analysis_variations {
                            let label = format!(
                                "d{} | nodes {} | eval {:+.2} | {:.2}s",
                                snapshot.depth,
                                format_number(snapshot.nodes),
                                snapshot.score_white as f32 / 100.0,
                                snapshot.elapsed_secs
                            );
                            if ui.button(label).clicked() {
                                self.status_message =
                                    format!("Selected variation: {}", snapshot.pv.join("  "));
                            }
                        }
                    });
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

    fn render_edit_controls(&mut self, ui: &mut egui::Ui) {
        if !self.edit_mode {
            return;
        }
        ui.separator();
        ui.heading("Edit Board");
        ui.label("Select a piece, then click another square to move it. Use piece tools for add/remove and promotions.");
        ui.label("Tip: click selected square again to clear it.");
        ui.horizontal_wrapped(|ui| {
            for tool in [
                EditTool::Move,
                EditTool::AddWhiteMan,
                EditTool::AddBlackMan,
                EditTool::AddWhiteKing,
                EditTool::AddBlackKing,
                EditTool::Remove,
                EditTool::ToggleKing,
            ] {
                ui.selectable_value(&mut self.edit_tool, tool, tool.label());
            }
        });
        ui.horizontal(|ui| {
            ui.label("Side to move:");
            ui.selectable_value(&mut self.edit_board.turn, Team::White, "White");
            ui.selectable_value(&mut self.edit_board.turn, Team::Black, "Black");
        });
        ui.horizontal(|ui| {
            if ui.button("Clear Board").clicked() {
                self.push_edit_undo();
                self.edit_board.state.pieces = [0, 0];
                self.edit_board.state.kings = 0;
                self.edit_selected = None;
                self.status_message = "Board cleared.".to_owned();
            }
            if ui.button("Reset Start Position").clicked() {
                self.push_edit_undo();
                self.edit_board = *Game::new().board();
                self.edit_selected = None;
                self.status_message = "Start position restored in edit mode.".to_owned();
            }
        });
        ui.horizontal(|ui| {
            if ui.button("Undo Edit").clicked() {
                self.undo_edit();
            }
            if ui.button("Redo Edit").clicked() {
                self.redo_edit();
            }
        });
        ui.horizontal(|ui| {
            if ui.button("Apply Position").clicked() {
                if let Err(err) = self.validate_edit_board() {
                    self.status_message = err;
                    return;
                }
                self.cancel_active_job();
                self.game = Game::from_board(self.edit_board);
                self.move_log.clear();
                self.latest_report = None;
                self.latest_purpose = None;
                self.analyzed_position = None;
                self.pending_choices.clear();
                self.selected = None;
                self.last_move = None;
                self.edit_mode = false;
                self.edit_selected = None;
                self.edit_undo_stack.clear();
                self.edit_redo_stack.clear();
                self.status_message =
                    "Custom position applied. Engine can start from this setup.".to_owned();
            }
            if ui.button("Cancel Edit").clicked() {
                self.edit_mode = false;
                self.edit_selected = None;
                self.edit_undo_stack.clear();
                self.edit_redo_stack.clear();
                self.status_message = "Edit mode canceled.".to_owned();
            }
        });
    }

    fn is_playable_square(_square: Square) -> bool {
        // Turkish draughts uses all board squares in play and setup.
        true
    }

    fn push_edit_undo(&mut self) {
        self.edit_undo_stack.push(self.edit_board);
        self.edit_redo_stack.clear();
    }

    fn undo_edit(&mut self) {
        if let Some(previous) = self.edit_undo_stack.pop() {
            self.edit_redo_stack.push(self.edit_board);
            self.edit_board = previous;
            self.edit_selected = None;
            self.status_message = "Edit undo applied.".to_owned();
        }
    }

    fn redo_edit(&mut self) {
        if let Some(next) = self.edit_redo_stack.pop() {
            self.edit_undo_stack.push(self.edit_board);
            self.edit_board = next;
            self.edit_selected = None;
            self.status_message = "Edit redo applied.".to_owned();
        }
    }

    fn validate_edit_board(&self) -> Result<(), String> {
        let white = self.edit_board.state.pieces[0];
        let black = self.edit_board.state.pieces[1];
        let kings = self.edit_board.state.kings;
        if white & black != 0 {
            return Err("Invalid position: overlapping white and black pieces.".to_owned());
        }
        if kings & !(white | black) != 0 {
            return Err("Invalid position: king bits must belong to existing pieces.".to_owned());
        }
        Ok(())
    }

    fn move_edit_piece(&mut self, from: Square, to: Square) {
        if from == to {
            self.edit_selected = None;
            return;
        }

        let from_mask = from.to_mask();
        let to_mask = to.to_mask();

        let white = self.edit_board.state.pieces[0] & from_mask != 0;
        let black = self.edit_board.state.pieces[1] & from_mask != 0;
        let king = self.edit_board.state.kings & from_mask != 0;

        if !white && !black {
            self.edit_selected = None;
            return;
        }
        if (self.edit_board.state.pieces[0] | self.edit_board.state.pieces[1]) & to_mask != 0 {
            self.edit_selected = Some(to);
            self.status_message =
                "Target square is occupied. Selected that piece instead.".to_owned();
            return;
        }
        self.push_edit_undo();

        self.edit_board.state.pieces[0] &= !from_mask;
        self.edit_board.state.pieces[1] &= !from_mask;
        self.edit_board.state.kings &= !from_mask;

        self.edit_board.state.pieces[0] &= !to_mask;
        self.edit_board.state.pieces[1] &= !to_mask;
        self.edit_board.state.kings &= !to_mask;

        if white {
            self.edit_board.state.pieces[0] |= to_mask;
        } else if black {
            self.edit_board.state.pieces[1] |= to_mask;
        }

        if king {
            self.edit_board.state.kings |= to_mask;
        }

        self.edit_selected = Some(to);
        self.status_message = format!("Moved piece from {} to {}.", from.to_usize(), to.to_usize());
    }

    fn click_edit_square(&mut self, square: Square) {
        if self.edit_tool != EditTool::Move {
            self.apply_edit_tool(square);
            return;
        }
        if let Some(selected) = self.edit_selected {
            if selected == square {
                self.edit_selected = None;
                return;
            }
            self.move_edit_piece(selected, square);
            return;
        }

        let mask = square.to_mask();
        let occupied =
            (self.edit_board.state.pieces[0] | self.edit_board.state.pieces[1]) & mask != 0;
        if occupied {
            self.edit_selected = Some(square);
        }
    }

    fn apply_edit_tool(&mut self, square: Square) {
        let mask = square.to_mask();
        let had_white = self.edit_board.state.pieces[0] & mask != 0;
        let had_black = self.edit_board.state.pieces[1] & mask != 0;
        let had_king = self.edit_board.state.kings & mask != 0;
        let had_piece = had_white || had_black;

        match self.edit_tool {
            EditTool::AddWhiteMan => {
                if had_white && !had_king {
                    return;
                }
                self.push_edit_undo();
                self.edit_board.state.pieces[0] &= !mask;
                self.edit_board.state.pieces[1] &= !mask;
                self.edit_board.state.kings &= !mask;
                self.edit_board.state.pieces[0] |= mask;
                self.status_message = "Placed white piece.".to_owned();
            }
            EditTool::AddBlackMan => {
                if had_black && !had_king {
                    return;
                }
                self.push_edit_undo();
                self.edit_board.state.pieces[0] &= !mask;
                self.edit_board.state.pieces[1] &= !mask;
                self.edit_board.state.kings &= !mask;
                self.edit_board.state.pieces[1] |= mask;
                self.status_message = "Placed black piece.".to_owned();
            }
            EditTool::AddWhiteKing => {
                if had_white && had_king {
                    return;
                }
                self.push_edit_undo();
                self.edit_board.state.pieces[0] &= !mask;
                self.edit_board.state.pieces[1] &= !mask;
                self.edit_board.state.kings &= !mask;
                self.edit_board.state.pieces[0] |= mask;
                self.edit_board.state.kings |= mask;
                self.status_message = "Placed white king.".to_owned();
            }
            EditTool::AddBlackKing => {
                if had_black && had_king {
                    return;
                }
                self.push_edit_undo();
                self.edit_board.state.pieces[0] &= !mask;
                self.edit_board.state.pieces[1] &= !mask;
                self.edit_board.state.kings &= !mask;
                self.edit_board.state.pieces[1] |= mask;
                self.edit_board.state.kings |= mask;
                self.status_message = "Placed black king.".to_owned();
            }
            EditTool::Remove => {
                if !had_piece {
                    return;
                }
                self.push_edit_undo();
                self.edit_board.state.pieces[0] &= !mask;
                self.edit_board.state.pieces[1] &= !mask;
                self.edit_board.state.kings &= !mask;
                self.status_message = "Removed piece.".to_owned();
            }
            EditTool::ToggleKing => {
                if had_piece {
                    self.push_edit_undo();
                    self.edit_board.state.kings ^= mask;
                    self.status_message = if had_king {
                        "Demoted king to man.".to_owned()
                    } else {
                        "Promoted man to king.".to_owned()
                    };
                } else {
                    self.status_message = "No piece on square to toggle king.".to_owned();
                }
            }
            EditTool::Move => {}
        }
        self.edit_selected = None;
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
        let board = if self.edit_mode {
            self.edit_board
        } else {
            *self.game.board()
        };
        let available = ui.available_size();
        let size = available.x.min(available.y).min(720.0).max(420.0);
        let cell = size / 8.0;
        let (board_rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());

        let painter = ui.painter();
        let legal_sources = if self.edit_mode {
            Vec::new()
        } else {
            self.legal_sources()
        };
        let targets = if self.edit_mode {
            Vec::new()
        } else {
            self.legal_targets()
        };

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

                if self.selected == Some(square)
                    || (self.edit_mode && self.edit_selected == Some(square))
                {
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
                    if self.edit_mode {
                        self.click_edit_square(square);
                    } else {
                        self.click_square(square);
                    }
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
        self.poll_engine_messages(ui.ctx());
        if !self.edit_mode {
            self.request_next_work(ui.ctx());
        }

        ui.add_space(8.0);
        self.render_header(ui);
        ui.add_space(12.0);
        ui.separator();
        ui.add_space(10.0);

        ui.columns(2, |columns| {
            columns[0].set_min_width(640.0);
            egui::Frame::new()
                .fill(Color32::from_rgb(20, 25, 33))
                .corner_radius(8.0)
                .inner_margin(egui::Margin::same(10))
                .show(&mut columns[0], |ui| {
                    self.render_board(ui);
                });

            let side_ctx = columns[1].ctx().clone();

            egui::Frame::new()
                .fill(Color32::from_rgb(20, 25, 33))
                .corner_radius(8.0)
                .inner_margin(egui::Margin::same(10))
                .show(&mut columns[1], |ui| {
                    ui.horizontal(|ui| {
                        for tab in [
                            SidePanelTab::Controls,
                            SidePanelTab::Analysis,
                            SidePanelTab::Moves,
                        ] {
                            let selected = self.side_panel_tab == tab;
                            let button = egui::Button::new(tab.label()).selected(selected);
                            if ui.add_sized([96.0, 28.0], button).clicked() {
                                self.side_panel_tab = tab;
                            }
                        }
                    });
                    ui.add_space(6.0);
                    ui.separator();
                    egui::ScrollArea::vertical().show(ui, |ui| match self.side_panel_tab {
                        SidePanelTab::Controls => {
                            self.render_controls(ui, &side_ctx);
                            self.render_edit_controls(ui);
                        }
                        SidePanelTab::Analysis => {
                            self.render_analysis(ui);
                        }
                        SidePanelTab::Moves => {
                            self.render_moves(ui);
                        }
                    });
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
