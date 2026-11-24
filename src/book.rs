//! Opening book: position key -> weighted candidate moves (stored as UHP strings).
//! Loaded from `thoraxrex-book.txt` next to the executable (or $TR_BOOK).
//! Line format: `<key hex> <weight> <move string>`.

use crate::board::*;
use std::collections::HashMap;
use std::sync::OnceLock;

type Book = HashMap<u64, Vec<(u32, String)>>;

fn book() -> &'static Book {
    static B: OnceLock<Book> = OnceLock::new();
    B.get_or_init(|| {
        let path = std::env::var("TR_BOOK").ok().map(std::path::PathBuf::from).or_else(|| {
            std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("thoraxrex-book.txt")))
        });
        let mut m: Book = HashMap::new();
        if let Some(p) = path {
            if let Ok(text) = std::fs::read_to_string(p) {
                for line in text.lines() {
                    let mut it = line.splitn(3, ' ');
                    let (Some(k), Some(w), Some(mv)) = (it.next(), it.next(), it.next()) else { continue };
                    let (Ok(k), Ok(w)) = (u64::from_str_radix(k, 16), w.parse::<u32>()) else { continue };
                    m.entry(k).or_default().push((w, mv.to_string()));
                }
            }
        }
        m
    })
}

fn rand_u32() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let n = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(7);
    let mut x = n ^ 0x9E37_79B9;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    x
}

/// One of the 12 symmetries of the hex grid about the first piece's cell: `t % 6` sixth-turns,
/// then (for t >= 6) a mirror.
fn sym(t: usize, q: i32, r: i32) -> (i32, i32) {
    let (mut q, mut r) = (q, r);
    for _ in 0..t % 6 {
        let nq = -r;
        r = q + r;
        q = nq;
    }
    if t >= 6 { (r, q) } else { (q, r) }
}

fn signed(x: u16) -> i32 {
    let v = (x & 31) as i32;
    if v >= 16 { v - 32 } else { v }
}

/// The book key this position would have after symmetry `t` (the same game played turned or mirrored).
pub fn sym_key(b: &Board, t: usize) -> u64 {
    let tb = tables();
    let mut k = 0u64;
    for p in 0..28 {
        let c = b.pos[p];
        if c == NONE {
            continue;
        }
        let (q, r) = sym(t, signed(c), signed(c >> 5));
        k ^= tb.zob[p][cell(q, r) as usize] ^ tb.zlvl[p][(b.lvl[p] as usize).min(7)];
    }
    if b.side() == 1 { k ^ tb.zside } else { k }
}

/// Direction index (E, NE, NW, W, SW, SE) after symmetry `t`.
fn sym_dir(t: usize, d: usize) -> usize {
    let (q, r) = sym(t, DQ[d], DR[d]);
    (0..6).find(|&i| DQ[i] == q && DR[i] == r).unwrap()
}

/// Rewrites a UHP move string with every direction mapped through `map`.
pub fn turn_move(mv: &str, map: &dyn Fn(usize) -> usize) -> String {
    let mut parts = mv.split_whitespace();
    let Some(piece) = parts.next() else { return mv.to_string() };
    let Some(tg) = parts.next() else { return mv.to_string() };
    let (dir, name) = match tg.chars().next() {
        Some('-') => (Some(3usize), &tg[1..]),
        Some('/') => (Some(4), &tg[1..]),
        Some('\\') => (Some(2), &tg[1..]),
        _ => match tg.chars().last() {
            Some('-') => (Some(0), &tg[..tg.len() - 1]),
            Some('/') => (Some(1), &tg[..tg.len() - 1]),
            Some('\\') => (Some(5), &tg[..tg.len() - 1]),
            _ => (None, tg),
        },
    };
    let target = match dir.map(map) {
        None => name.to_string(),
        Some(0) => format!("{}-", name),
        Some(1) => format!("{}/", name),
        Some(2) => format!("\\{}", name),
        Some(3) => format!("-{}", name),
        Some(4) => format!("/{}", name),
        _ => format!("{}\\", name),
    };
    format!("{} {}", piece, target)
}

/// The same move after symmetry `t` (used by tests and tools).
pub fn sym_move(mv: &str, t: usize) -> String {
    turn_move(mv, &|d| sym_dir(t, d))
}

/// Pick a weighted book move for the current position, if any (validated legal). The book is also
/// consulted for the position turned or mirrored, so one stored line covers all 12 orientations.
pub fn probe(b: &mut Board) -> Option<Move> {
    if book().is_empty() {
        return None;
    }
    for t in 0..12 {
        let key = if t == 0 { b.book_key() } else { sym_key(b, t) };
        let Some(entries) = book().get(&key) else { continue };
        let total: u32 = entries.iter().map(|e| e.0).sum();
        if total == 0 {
            continue;
        }
        let mut r = rand_u32() % total;
        let mut pick = &entries[0].1;
        for (w, mv) in entries {
            if r < *w {
                pick = mv;
                break;
            }
            r -= w;
        }
        // The stored move is for the turned position: turn it back.
        let mv = if t == 0 { pick.clone() } else { turn_move(pick, &|d| (0..6).find(|&i| sym_dir(t, i) == d).unwrap()) };
        if let Ok(m) = b.parse_move(&mv) {
            return Some(m);
        }
    }
    None
}

/// True when an opening book file was found and loaded.
pub fn has_book() -> bool {
    !book().is_empty()
}
