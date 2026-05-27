use kish::{Game, GameStatus, Team};
use kish_dama_studio::engine::{Engine, EngineConfig};
use std::sync::{atomic::AtomicBool, Arc};
use std::time::Duration;

#[derive(Clone, Copy)]
struct SideConfig {
    depth: u32,
    millis: u64,
}

#[derive(Default)]
struct Score {
    wins: u64,
    draws: u64,
    losses: u64,
}

fn main() {
    let games: u64 = std::env::var("GAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(40);
    let base = SideConfig {
        depth: 8,
        millis: 200,
    };
    let test = SideConfig {
        depth: 9,
        millis: 200,
    };

    let mut score = Score::default();
    for round in 0..games {
        let flip = round % 2 == 1;
        let (white_cfg, black_cfg) = if !flip { (test, base) } else { (base, test) };
        let result = play_one(white_cfg, black_cfg);
        match result {
            Some(Team::White) if !flip => score.wins += 1,
            Some(Team::Black) if flip => score.wins += 1,
            Some(Team::White) if flip => score.losses += 1,
            Some(Team::Black) if !flip => score.losses += 1,
            _ => score.draws += 1,
        }
    }

    println!("RESULT,{},{},{}", score.wins, score.draws, score.losses);
    let llr = sprt_llr(
        score.wins as f64,
        score.draws as f64,
        score.losses as f64,
        0.0,
        10.0,
    );
    println!("SPRT_LLR,{:.4}", llr);
}

fn play_one(white: SideConfig, black: SideConfig) -> Option<Team> {
    let mut game = Game::new();
    let cancel = Arc::new(AtomicBool::new(false));

    let max_plies = 180;
    for _ in 0..max_plies {
        if game.board().actions().is_empty() {
            break;
        }
        let board = *game.board();
        let cfg = if board.turn == Team::White {
            white
        } else {
            black
        };
        let engine_cfg = EngineConfig::with_limits(
            Some(cfg.depth),
            Some(Duration::from_millis(cfg.millis)),
            None,
        );
        let mut engine = Engine::new(engine_cfg, Arc::clone(&cancel), board);
        let report = engine.search(|_| {});
        let Some(best) = report.and_then(|r| r.best_action) else {
            break;
        };
        game.apply(best.to_detailed(board.turn, &board.state)).ok();

        match game.status() {
            GameStatus::InProgress => {}
            GameStatus::Draw => return None,
            GameStatus::Won(team) => return Some(team),
        }
    }
    None
}

fn logistic(elo: f64) -> f64 {
    1.0 / (1.0 + 10f64.powf(-elo / 400.0))
}

fn sprt_llr(w: f64, d: f64, l: f64, elo0: f64, elo1: f64) -> f64 {
    let n = w + d + l;
    if n == 0.0 {
        return 0.0;
    }
    let p_hat = (w + 0.5 * d) / n;
    let p0 = logistic(elo0);
    let p1 = logistic(elo1);
    let eps = 1e-12;
    let p_hat = p_hat.clamp(eps, 1.0 - eps);
    n * (p_hat * (p1 / p0).ln() + (1.0 - p_hat) * ((1.0 - p1) / (1.0 - p0)).ln())
}
