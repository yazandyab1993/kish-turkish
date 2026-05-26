use kish::{Action, Board, Game, Square, Team};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariationBook {
    pub metadata: VariationMetadata,
    pub entries: HashMap<String, MoveSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariationMetadata {
    pub source_path: String,
    pub loaded_lines: usize,
    pub rejected_lines: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveSpec {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone)]
struct ParsedMove {
    from: Square,
    to: Square,
}

#[derive(Debug)]
pub enum VariationLoadError {
    Read(std::io::Error),
}

impl std::fmt::Display for VariationLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read(err) => write!(f, "failed to read variation file: {err}"),
        }
    }
}

pub fn load_variation_book(
    path: &Path,
) -> Result<(VariationBook, Vec<String>), VariationLoadError> {
    let content = std::fs::read_to_string(path).map_err(VariationLoadError::Read)?;
    let mut entries = HashMap::new();
    let mut warnings = Vec::new();
    let mut loaded_lines = 0;
    let mut rejected_lines = 0;

    for (line_idx, raw_line) in content.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.is_empty() {
            continue;
        }

        let parsed_moves: Result<Vec<ParsedMove>, String> =
            tokens.iter().map(|token| parse_move_token(token)).collect();

        let parsed_moves = match parsed_moves {
            Ok(moves) => moves,
            Err(err) => {
                rejected_lines += 1;
                warnings.push(format!("line {} rejected: {err}", line_idx + 1));
                continue;
            }
        };

        let mut game = Game::new();
        let mut line_valid = true;

        for parsed in parsed_moves {
            let board = *game.board();
            let side = board.turn;
            let Some(action) = find_action_for_move(board, parsed.from, parsed.to) else {
                line_valid = false;
                warnings.push(format!(
                    "line {} rejected: illegal move {}-{} for current position",
                    line_idx + 1,
                    square_name(parsed.from),
                    square_name(parsed.to)
                ));
                break;
            };

            let key = board_key(board);
            entries.entry(key).or_insert_with(|| MoveSpec {
                from: square_name(action.source(side, board.friendly_pieces())),
                to: square_name(action.destination(side, board.friendly_pieces())),
            });

            game.apply(&action);
        }

        if line_valid {
            loaded_lines += 1;
        } else {
            rejected_lines += 1;
        }
    }

    Ok((
        VariationBook {
            metadata: VariationMetadata {
                source_path: path.display().to_string(),
                loaded_lines,
                rejected_lines,
            },
            entries,
        },
        warnings,
    ))
}

pub fn lookup_action(book: &VariationBook, board: Board) -> Option<MoveSpec> {
    book.entries.get(&board_key(board)).cloned()
}

fn parse_move_token(token: &str) -> Result<ParsedMove, String> {
    let mut parts = token.split('-');
    let from = parts
        .next()
        .ok_or_else(|| format!("invalid token '{token}'"))?;
    let to = parts
        .next()
        .ok_or_else(|| format!("invalid token '{token}'"))?;
    if parts.next().is_some() {
        return Err(format!("invalid token '{token}'"));
    }

    Ok(ParsedMove {
        from: parse_square(from)?,
        to: parse_square(to)?,
    })
}

fn parse_square(value: &str) -> Result<Square, String> {
    let mut chars = value.chars();
    let file = chars
        .next()
        .ok_or_else(|| format!("invalid square '{value}'"))?;
    let rank = chars
        .next()
        .ok_or_else(|| format!("invalid square '{value}'"))?;
    if chars.next().is_some() {
        return Err(format!("invalid square '{value}'"));
    }

    if !(('a'..='h').contains(&file)) {
        return Err(format!("invalid file in square '{value}'"));
    }
    if !(('1'..='8').contains(&rank)) {
        return Err(format!("invalid rank in square '{value}'"));
    }

    let file_idx = (file as u8 - b'a') as usize;
    let rank_idx = rank.to_digit(10).unwrap() as usize;
    let row = 8 - rank_idx;
    let idx = row * 8 + file_idx;
    Square::try_from_usize(idx).ok_or_else(|| format!("invalid square index for '{value}'"))
}

fn square_name(square: Square) -> String {
    let idx = square.to_usize();
    let row = idx / 8;
    let col = idx % 8;
    let file = (b'a' + col as u8) as char;
    let rank = 8 - row;
    format!("{}{}", file, rank)
}

fn board_key(board: Board) -> String {
    format!(
        "{}|{}",
        match board.turn {
            Team::White => "w",
            Team::Black => "b",
        },
        board.state
    )
}

fn find_action_for_move(board: Board, from: Square, to: Square) -> Option<Action> {
    board.actions().into_iter().find(|action| {
        action.source(board.turn, board.friendly_pieces()) == from
            && action.destination(board.turn, board.friendly_pieces()) == to
    })
}
