use kish::{Game, GameStatus, Team};
use kish_dama_studio::engine::{Engine, EngineConfig};
use std::fs::File;
use std::io::{BufWriter, Write};
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
    let games: u64 = read_env_u64("GAMES", 100);
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "balanced".to_string());
    let (base, test) = match profile.as_str() {
        "fast" => (
            SideConfig {
                depth: 7,
                millis: 80,
            },
            SideConfig {
                depth: 8,
                millis: 80,
            },
        ),
        "deep" => (
            SideConfig {
                depth: 10,
                millis: 350,
            },
            SideConfig {
                depth: 11,
                millis: 350,
            },
        ),
        _ => (
            SideConfig {
                depth: 8,
                millis: 150,
            },
            SideConfig {
                depth: 9,
                millis: 150,
            },
        ),
    };

    let log_path = std::env::var("CSV_LOG").ok();
    let mut csv = log_path.and_then(|path| File::create(path).ok().map(BufWriter::new));
    if let Some(w) = csv.as_mut() {
        let _ = writeln!(w, "game,flip,result,plies");
    }

    let mut score = Score::default();
    for round in 0..games {
        let flip = round % 2 == 1;
        let (white_cfg, black_cfg) = if !flip { (test, base) } else { (base, test) };
        let (result, plies) = play_one(white_cfg, black_cfg);
        match result {
            Some(Team::White) if !flip => score.wins += 1,
            Some(Team::Black) if flip => score.wins += 1,
            Some(Team::White) if flip => score.losses += 1,
            Some(Team::Black) if !flip => score.losses += 1,
            _ => score.draws += 1,
        }

        if let Some(w) = csv.as_mut() {
            let result_str = match result {
                Some(Team::White) => "W",
                Some(Team::Black) => "B",
                None => "D",
            };
            let _ = writeln!(w, "{round},{flip},{result_str},{plies}");
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
    println!("SPRT_LLR,{llr:.4}");

    let (lower, upper) = sprt_boundaries(0.05, 0.05);
    let decision = if llr >= upper {
        "ACCEPT_H1"
    } else if llr <= lower {
        "ACCEPT_H0"
    } else {
        "CONTINUE"
    };
    println!("SPRT_BOUNDS,{lower:.4},{upper:.4}");
    println!("SPRT_DECISION,{decision}");
}

fn read_env_u64(key: &str, default_value: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(default_value)
}

fn play_one(white: SideConfig, black: SideConfig) -> (Option<Team>, u32) {
    let mut game = Game::new();
    let cancel = Arc::new(AtomicBool::new(false));

    let max_plies = 180;
    for ply in 1..=max_plies {
        if game.board().actions().is_empty() {
            return (None, ply);
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
            return (None, ply);
        };

        if game
            .apply(best.to_detailed(board.turn, &board.state))
            .is_err()
        {
            return (None, ply);
        }

        match game.status() {
            GameStatus::InProgress => {}
            GameStatus::Draw => return (None, ply),
            GameStatus::Won(team) => return (Some(team), ply),
        }
    }

    (None, max_plies)
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

fn sprt_boundaries(alpha: f64, beta: f64) -> (f64, f64) {
    ((beta / (1.0 - alpha)).ln(), ((1.0 - beta) / alpha).ln())
}
