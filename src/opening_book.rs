use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const BOARD_SIZE: usize = 8;
pub const DEFAULT_MAX_PLY: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    White,
    Black,
}

impl Side {
    fn opposite(self) -> Self {
        match self {
            Self::White => Self::Black,
            Self::Black => Self::White,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::White => "white",
            Self::Black => "black",
        }
    }
}

pub type Board = [[char; BOARD_SIZE]; BOARD_SIZE];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct MoveRecord {
    pub from: String,
    pub to: String,
    pub captures: Vec<String>,
    pub promotion: bool,
    pub side_to_move: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookMoveEntry {
    pub r#move: MoveRecord,
    pub count: u32,
    pub avg_time: f64,
    pub captures_count: u32,
    pub first_seen_file: String,
    pub ply_index: usize,
    pub score: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpeningBook {
    pub metadata: Metadata,
    pub book: BTreeMap<String, Vec<BookMoveEntry>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Metadata {
    pub source_files: usize,
    pub max_ply: usize,
    pub positions: usize,
    pub generated_at: String,
}

#[derive(Default, Debug)]
pub struct BuildStats {
    pub files_count: usize,
    pub games_read: usize,
    pub positions: usize,
    pub extracted_moves: usize,
    pub rejected_moves: usize,
}

#[derive(Debug)]
struct CsvRow { time: f64, board: Board }

#[derive(Debug, Clone)]
struct AggregatedMove {
    move_record: MoveRecord,
    count: u32,
    time_sum: f64,
    captures_count: u32,
    first_seen_file: String,
    ply_index: usize,
}

pub fn parse_board(raw: &str) -> Option<Board> {
    let mut board = [['-'; BOARD_SIZE]; BOARD_SIZE];
    let rows: Vec<&str> = raw.split_whitespace().collect();
    if rows.len() != BOARD_SIZE { return None; }
    for (r, row) in rows.iter().enumerate() {
        if row.chars().count() != BOARD_SIZE { return None; }
        for (c, ch) in row.chars().enumerate() {
            if !matches!(ch, 'w' | 'W' | 'b' | 'B' | '-') { return None; }
            board[r][c] = ch;
        }
    }
    Some(board)
}

fn square_name(r: usize, c: usize) -> String {
    let file = (b'a' + c as u8) as char;
    let rank = (BOARD_SIZE - r).to_string();
    format!("{}{}", file, rank)
}

pub fn infer_move(prev: &Board, next: &Board, side_hint: Option<Side>) -> Option<MoveRecord> {
    let mut from = None;
    let mut to = None;
    let mut captures = Vec::new();
    let mut promotion = false;

    for r in 0..BOARD_SIZE {
        for c in 0..BOARD_SIZE {
            let p = prev[r][c];
            let n = next[r][c];
            if p == n { continue; }
            if n == '-' && matches!(p, 'w'|'W'|'b'|'B') {
                from = Some((r,c,p));
            } else if p == '-' && matches!(n, 'w'|'W'|'b'|'B') {
                to = Some((r,c,n));
            } else if matches!(p, 'w'|'W'|'b'|'B') && n == '-' {
                captures.push(square_name(r,c));
            } else {
                if p != '-' && n != '-' {
                    from = Some((r,c,p));
                    to = Some((r,c,n));
                }
            }
        }
    }

    let (fr, fc, fp) = from?;
    let (tr, tc, tp) = to?;
    if fp.eq_ignore_ascii_case(&tp) && fp != tp { promotion = true; }
    if fp == 'w' && tp == 'W' || fp == 'b' && tp == 'B' { promotion = true; }

    let side = side_hint.map(|s| s.as_str().to_owned()).or_else(|| {
        if matches!(fp, 'w'|'W') { Some("white".to_string()) } else if matches!(fp, 'b'|'B') { Some("black".to_string()) } else { None }
    });

    Some(MoveRecord {
        from: square_name(fr,fc),
        to: square_name(tr,tc),
        captures,
        promotion,
        side_to_move: side,
    })
}

pub fn position_hash(board: &Board, side: Side) -> String {
    let mut h: u64 = 1469598103934665603;
    for r in 0..BOARD_SIZE {
        for c in 0..BOARD_SIZE {
            h ^= board[r][c] as u64;
            h = h.wrapping_mul(1099511628211);
            h ^= ((r * BOARD_SIZE + c) as u64).wrapping_mul(31);
        }
    }
    h ^= match side { Side::White => 0xABCDEF1234, Side::Black => 0x1234ABCDEF };
    format!("{:016x}", h)
}

pub fn build_opening_book(training_dir: &Path, max_ply: usize, output: &Path, rejected_log: &Path) -> std::io::Result<BuildStats> {
    let mut stats = BuildStats::default();
    let mut rejected = File::create(rejected_log)?;

    let mut files: Vec<PathBuf> = fs::read_dir(training_dir)?
        .filter_map(|e| e.ok().map(|x| x.path()))
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("csv"))
        .collect();
    files.sort();
    stats.files_count = files.len();

    let mut book_map: HashMap<String, HashMap<MoveRecord, AggregatedMove>> = HashMap::new();

    for file in files {
        let rows = parse_game_file(&file)?;
        if rows.len() < 2 { continue; }
        stats.games_read += 1;
        let mut side = Side::White;
        for ply in 0..rows.len().saturating_sub(1).min(max_ply) {
            let prev = &rows[ply];
            let next = &rows[ply + 1];
            let hash = position_hash(&prev.board, side);
            let Some(mv) = infer_move(&prev.board, &next.board, Some(side)) else {
                stats.rejected_moves += 1;
                writeln!(rejected, "{} | ply={} | unable to infer move", file.display(), ply)?;
                side = side.opposite();
                continue;
            };

            let slot = book_map.entry(hash).or_default();
            let e = slot.entry(mv.clone()).or_insert_with(|| AggregatedMove {
                move_record: mv.clone(),
                count: 0,
                time_sum: 0.0,
                captures_count: 0,
                first_seen_file: file.file_name().and_then(|s| s.to_str()).unwrap_or("unknown").to_string(),
                ply_index: ply,
            });
            e.count += 1;
            e.time_sum += next.time.max(0.0);
            e.captures_count += mv.captures.len() as u32;
            stats.extracted_moves += 1;
            side = side.opposite();
        }
    }

    let mut book = BTreeMap::new();
    for (position, moves_map) in book_map {
        let mut moves: Vec<BookMoveEntry> = moves_map.into_values().map(|m| {
            let avg_time = if m.count == 0 { 0.0 } else { m.time_sum / m.count as f64 };
            let score = m.count as f64 * 100.0 + m.captures_count as f64 * 10.0 - avg_time;
            BookMoveEntry {
                r#move: m.move_record,
                count: m.count,
                avg_time,
                captures_count: m.captures_count,
                first_seen_file: m.first_seen_file,
                ply_index: m.ply_index,
                score,
            }
        }).collect();
        moves.sort_by(|a,b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        book.insert(position, moves);
    }

    stats.positions = book.len();
    let generated_at = format!("{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs());
    let opening = OpeningBook { metadata: Metadata { source_files: stats.files_count, max_ply, positions: stats.positions, generated_at }, book };
    fs::write(output, serde_json::to_string_pretty(&opening).unwrap())?;
    Ok(stats)
}

fn parse_game_file(path: &Path) -> std::io::Result<Vec<CsvRow>> {
    let file = File::open(path)?;
    let mut out = Vec::new();
    for (i,line) in BufReader::new(file).lines().enumerate() {
        let line = line?;
        if i == 0 && line.contains("AUTO") { continue; }
        let parts: Vec<&str> = line.splitn(3, ',').collect();
        if parts.len() != 3 { continue; }
        let time = parts[1].trim().parse::<f64>().unwrap_or(0.0);
        if let Some(board) = parse_board(parts[2].trim()) { out.push(CsvRow { time, board }); }
    }
    Ok(out)
}

pub fn get_book_move(book: &OpeningBook, board: &Board, side: Side, strategy: &str) -> Option<MoveRecord> {
    let key = position_hash(board, side);
    let moves = book.book.get(&key)?;
    if moves.is_empty() { return None; }
    match strategy {
        "popular" => moves.iter().max_by_key(|m| m.count).map(|m| m.r#move.clone()),
        "random_weighted" => {
            let sum: u32 = moves.iter().map(|m| m.count).sum();
            if sum == 0 { return Some(moves[0].r#move.clone()); }
            let mut state = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().subsec_nanos() % sum;
            for m in moves {
                if state < m.count { return Some(m.r#move.clone()); }
                state -= m.count;
            }
            Some(moves[0].r#move.clone())
        }
        _ => moves.iter().max_by(|a,b| a.score.partial_cmp(&b.score).unwrap_or(std::cmp::Ordering::Equal)).map(|m| m.r#move.clone()),
    }
}
