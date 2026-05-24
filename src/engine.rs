use kish::{Action, Board, GameStatus, Team};
use std::cmp::Reverse;
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

const INF: i32 = 2_000_000_000;
const MATE_SCORE: i32 = 1_000_000;
const CENTER_BOX: u64 = 0x0000_3C3C_3C3C_0000;

#[derive(Clone, Copy, Debug)]
enum Bound {
    Exact,
    Lower,
    Upper,
}

#[derive(Clone, Copy, Debug)]
struct TTEntry {
    depth: u32,
    score: i32,
    bound: Bound,
    best_action: Option<Action>,
}

#[derive(Clone, Copy, Debug)]
pub struct EngineConfig {
    pub max_depth: u32,
    pub max_time: Duration,
    pub tt_max_entries: usize,
    pub tt_initial_capacity: usize,
}

impl EngineConfig {
    pub fn play(max_depth: u32, seconds: u64) -> Self {
        Self {
            max_depth,
            max_time: Duration::from_secs(seconds),
            tt_max_entries: 2_000_000,
            tt_initial_capacity: 1_000_000,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SearchReport {
    pub best_action: Option<Action>,
    pub score_white: i32,
    pub completed_depth: u32,
    pub nodes: u64,
    pub qnodes: u64,
    pub tt_hits: u64,
    pub cutoffs: u64,
    pub elapsed: Duration,
    pub nps: u64,
    pub tt_entries: usize,
    pub principal_variation: Vec<String>,
    pub forced_root: bool,
}

#[derive(Debug)]
struct SearchInterrupted;

pub struct Engine {
    config: EngineConfig,
    start: Instant,
    root_board: Board,
    nodes: u64,
    qnodes: u64,
    tt_hits: u64,
    cutoffs: u64,
    tt: HashMap<Board, TTEntry>,
    cancel: Arc<AtomicBool>,
}

impl Engine {
    pub fn new(config: EngineConfig, cancel: Arc<AtomicBool>, root_board: Board) -> Self {
        Self {
            config,
            start: Instant::now(),
            root_board,
            nodes: 0,
            qnodes: 0,
            tt_hits: 0,
            cutoffs: 0,
            tt: HashMap::with_capacity(config.tt_initial_capacity),
            cancel,
        }
    }

    pub fn search<F>(&mut self, mut on_depth_complete: F) -> Option<SearchReport>
    where
        F: FnMut(SearchReport),
    {
        self.start = Instant::now();
        self.nodes = 0;
        self.qnodes = 0;
        self.tt_hits = 0;
        self.cutoffs = 0;
        self.tt.clear();

        if self.root_board.actions().is_empty() {
            return None;
        }

        let mut latest: Option<SearchReport> = None;

        for depth in 1..=self.config.max_depth {
            if self.should_stop() {
                break;
            }

            match self.search_root(depth) {
                Ok((score, best_action)) => {
                    let report = self.make_report(depth, score, best_action);
                    on_depth_complete(report.clone());
                    latest = Some(report);
                }
                Err(SearchInterrupted) => break,
            }
        }

        latest
    }

    fn make_report(&self, depth: u32, score_from_root_turn: i32, action: Action) -> SearchReport {
        let elapsed = self.start.elapsed();
        let secs = elapsed.as_secs_f64().max(0.000_001);
        let score_white = if self.root_board.turn == Team::White {
            score_from_root_turn
        } else {
            -score_from_root_turn
        };

        SearchReport {
            best_action: Some(action),
            score_white,
            completed_depth: depth,
            nodes: self.nodes,
            qnodes: self.qnodes,
            tt_hits: self.tt_hits,
            cutoffs: self.cutoffs,
            elapsed,
            nps: (self.nodes as f64 / secs) as u64,
            tt_entries: self.tt.len(),
            principal_variation: self.extract_pv(self.root_board, depth),
            forced_root: Self::is_forced_capture_position(self.root_board),
        }
    }

    fn search_root(&mut self, depth: u32) -> Result<(i32, Action), SearchInterrupted> {
        self.ensure_running()?;

        let board = self.root_board;
        let mut actions = board.actions();
        if actions.is_empty() {
            return Err(SearchInterrupted);
        }

        let forced_capture = Self::is_forced_capture_actions(board, &actions);
        if actions.len() == 1 && forced_capture {
            let only_action = actions[0];
            let child = board.apply(&only_action).swap_turn();
            let score = -self.negamax(child, depth.saturating_sub(1), -INF, INF, 1)?;
            self.store_tt(
                board,
                TTEntry {
                    depth,
                    score,
                    bound: Bound::Exact,
                    best_action: Some(only_action),
                },
            );
            return Ok((score, only_action));
        }

        let tt_move = self.tt.get(&board).and_then(|entry| entry.best_action);
        self.order_moves(board, &mut actions, tt_move);

        let mut best_score = -INF;
        let mut best_tie_break = -INF;
        let mut best_action = actions[0];

        for action in actions {
            self.ensure_running()?;

            let child = board.apply(&action).swap_turn();
            let score = -self.negamax(child, depth.saturating_sub(1), -INF, INF, 1)?;
            let tie_break = if forced_capture {
                action.capture_count(board.turn) as i32
            } else {
                self.root_move_bonus(board, &action)
            };

            if score > best_score || (score == best_score && tie_break > best_tie_break) {
                best_score = score;
                best_tie_break = tie_break;
                best_action = action;
            }
        }

        self.store_tt(
            board,
            TTEntry {
                depth,
                score: best_score,
                bound: Bound::Exact,
                best_action: Some(best_action),
            },
        );

        Ok((best_score, best_action))
    }

    fn negamax(
        &mut self,
        board: Board,
        depth: u32,
        mut alpha: i32,
        mut beta: i32,
        ply: u32,
    ) -> Result<i32, SearchInterrupted> {
        self.visit_node()?;

        if let Some(score) = self.terminal_score(board, ply) {
            return Ok(score);
        }

        if depth == 0 {
            return self.quiescence(board, alpha, beta, ply);
        }

        let alpha_start = alpha;
        let beta_start = beta;

        let mut tt_move: Option<Action> = None;
        if let Some(entry) = self.tt.get(&board).copied() {
            tt_move = entry.best_action;
            if entry.depth >= depth {
                self.tt_hits += 1;
                match entry.bound {
                    Bound::Exact => return Ok(entry.score),
                    Bound::Lower => alpha = alpha.max(entry.score),
                    Bound::Upper => beta = beta.min(entry.score),
                }
                if alpha >= beta {
                    return Ok(entry.score);
                }
            }
        }

        let mut actions = board.actions();
        if actions.is_empty() {
            return Ok(-MATE_SCORE + ply as i32);
        }
        let forced_capture = Self::is_forced_capture_actions(board, &actions);
        self.order_moves(board, &mut actions, tt_move);

        let mut best_score = -INF;
        let mut best_action: Option<Action> = None;

        for action in actions {
            let child = board.apply(&action).swap_turn();
            let next_depth = if forced_capture { depth } else { depth - 1 };
            let score = -self.negamax(child, next_depth, -beta, -alpha, ply + 1)?;

            if score > best_score {
                best_score = score;
                best_action = Some(action);
            }
            if score > alpha {
                alpha = score;
            }
            if alpha >= beta {
                self.cutoffs += 1;
                break;
            }
        }

        let bound = if best_score <= alpha_start {
            Bound::Upper
        } else if best_score >= beta_start {
            Bound::Lower
        } else {
            Bound::Exact
        };

        self.store_tt(
            board,
            TTEntry {
                depth,
                score: best_score,
                bound,
                best_action,
            },
        );

        Ok(best_score)
    }

    fn quiescence(
        &mut self,
        board: Board,
        mut alpha: i32,
        beta: i32,
        ply: u32,
    ) -> Result<i32, SearchInterrupted> {
        self.visit_node()?;
        self.qnodes += 1;

        if let Some(score) = self.terminal_score(board, ply) {
            return Ok(score);
        }

        let mut actions = board.actions();
        let forced_capture = actions
            .first()
            .map(|action| action.is_capture(board.turn))
            .unwrap_or(false);

        if !forced_capture {
            return Ok(self.evaluate_for_turn(board));
        }

        self.order_moves(board, &mut actions, None);
        let mut best_score = -INF;

        for action in actions {
            let child = board.apply(&action).swap_turn();
            let score = -self.quiescence(child, -beta, -alpha, ply + 1)?;

            if score > best_score {
                best_score = score;
            }
            if score > alpha {
                alpha = score;
            }
            if alpha >= beta {
                self.cutoffs += 1;
                return Ok(alpha);
            }
        }

        Ok(best_score)
    }

    fn terminal_score(&self, board: Board, ply: u32) -> Option<i32> {
        match board.status() {
            GameStatus::InProgress => None,
            GameStatus::Draw => Some(0),
            GameStatus::Won(winner) => {
                if winner == board.turn {
                    Some(MATE_SCORE - ply as i32)
                } else {
                    Some(-MATE_SCORE + ply as i32)
                }
            }
        }
    }

    fn evaluate_for_turn(&self, board: Board) -> i32 {
        let white_score = Self::evaluate_white_static(board);
        if board.turn == Team::White {
            white_score
        } else {
            -white_score
        }
    }

    pub fn board_cache_key(board: Board) -> String {
        format!(
            "turn={:?}|pieces={:016x}-{:016x}|kings={:016x}",
            board.turn, board.state.pieces[0], board.state.pieces[1], board.state.kings
        )
    }

    pub fn evaluate_white_static(board: Board) -> i32 {
        let white = board.state.pieces[0];
        let black = board.state.pieces[1];
        let kings = board.state.kings;

        let white_kings = white & kings;
        let black_kings = black & kings;
        let white_men = white & !kings;
        let black_men = black & !kings;

        let mut score = 0;

        score += white_men.count_ones() as i32 * 100;
        score -= black_men.count_ones() as i32 * 100;
        score += white_kings.count_ones() as i32 * 360;
        score -= black_kings.count_ones() as i32 * 360;

        score += Self::advancement_score(white_men, true);
        score -= Self::advancement_score(black_men, false);

        score += (white & CENTER_BOX).count_ones() as i32 * 8;
        score -= (black & CENTER_BOX).count_ones() as i32 * 8;

        score += (white_kings & CENTER_BOX).count_ones() as i32 * 6;
        score -= (black_kings & CENTER_BOX).count_ones() as i32 * 6;

        score
    }

    fn advancement_score(mut men: u64, is_white: bool) -> i32 {
        let mut score = 0;
        while men != 0 {
            let index = men.trailing_zeros() as i32;
            let row = index / 8;
            let advancement = if is_white { row } else { 7 - row };
            score += advancement * 6;
            if advancement == 6 {
                score += 28;
            }
            men &= men - 1;
        }
        score
    }

    fn root_move_bonus(&self, board: Board, action: &Action) -> i32 {
        let destination = action.destination(board.turn, board.friendly_pieces());
        let col = destination.column() as i32;
        let row = destination.row() as i32;

        let central_bonus = match col {
            3 | 4 => 10,
            2 | 5 => 7,
            1 | 6 => 3,
            _ => 0,
        };

        let forward_progress = match board.turn {
            Team::White => row,
            Team::Black => 7 - row,
        };

        let mut bonus = central_bonus + forward_progress;
        if action.is_promotion(board.turn, &board.state) {
            bonus += 100;
        }
        if action.is_capture(board.turn) {
            bonus += 200 + action.capture_count(board.turn) as i32 * 40;
        }
        bonus
    }

    fn order_moves(&self, board: Board, actions: &mut Vec<Action>, preferred: Option<Action>) {
        actions.sort_unstable_by_key(|action| {
            Reverse(self.move_order_score(board, action, preferred))
        });
    }

    fn move_order_score(&self, board: Board, action: &Action, preferred: Option<Action>) -> i32 {
        let mut score = 0;

        if preferred == Some(*action) {
            score += 1_000_000;
        }
        if action.is_capture(board.turn) {
            score += 100_000 + action.capture_count(board.turn) as i32 * 10_000;
        }
        if action.is_promotion(board.turn, &board.state) {
            score += 50_000;
        }

        let destination = action.destination(board.turn, board.friendly_pieces());
        let col = destination.column() as i32;
        let row = destination.row() as i32;
        score += match col {
            3 | 4 => 30,
            2 | 5 => 20,
            1 | 6 => 10,
            _ => 0,
        };
        score += match board.turn {
            Team::White => row,
            Team::Black => 7 - row,
        };
        score
    }

    fn is_forced_capture_position(board: Board) -> bool {
        let actions = board.actions();
        Self::is_forced_capture_actions(board, &actions)
    }

    fn is_forced_capture_actions(board: Board, actions: &[Action]) -> bool {
        actions
            .first()
            .map(|action| action.is_capture(board.turn))
            .unwrap_or(false)
    }

    fn extract_pv(&self, mut board: Board, max_depth: u32) -> Vec<String> {
        let mut pv = Vec::new();
        for _ in 0..max_depth {
            let Some(entry) = self.tt.get(&board) else {
                break;
            };
            let Some(action) = entry.best_action else {
                break;
            };
            let notation = action.to_detailed(board.turn, &board.state).to_notation();
            pv.push(notation);
            board = board.apply(&action).swap_turn();
        }
        pv
    }

    fn store_tt(&mut self, board: Board, entry: TTEntry) {
        if self.tt.len() < self.config.tt_max_entries || self.tt.contains_key(&board) {
            self.tt.insert(board, entry);
        }
    }

    fn visit_node(&mut self) -> Result<(), SearchInterrupted> {
        self.nodes += 1;
        if (self.nodes & 2047) == 0 {
            self.ensure_running()?;
        }
        Ok(())
    }

    fn ensure_running(&self) -> Result<(), SearchInterrupted> {
        if self.should_stop() {
            Err(SearchInterrupted)
        } else {
            Ok(())
        }
    }

    fn should_stop(&self) -> bool {
        self.cancel.load(Ordering::Relaxed) || self.start.elapsed() >= self.config.max_time
    }
}
