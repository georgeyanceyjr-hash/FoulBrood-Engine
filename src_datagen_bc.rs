//! Self-play data generation: `thoraxrex datagen <games> <depth> <seed> <out.jsonl>`.
//! Each game starts with 4-8 random plies, then both sides play a fixed-depth search.
//! Writes one JSON line per game: moves, per-ply search scores (side to move), result.
use crate::board::*;
use crate::search::{Limits, Searcher};
use std::io::Write;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

pub fn run(args: &[String]) {
    let games: usize = args.get(0).and_then(|s| s.parse().ok()).unwrap_or(10);
    let depth: u32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(5);
    let seed: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1);
    let out = args.get(3).cloned().unwrap_or_else(|| "datagen.jsonl".into());
    let gt = GameType::parse("Base+MLP").unwrap();
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&out).expect("open output");
    let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let mut searcher = Searcher::new(32);
    let mut legal = Vec::new();
    // TR_STARTS=<games.jsonl>: start each game from a random prefix (8..40 plies) of a real game.
    let starts: Vec<Vec<String>> = std::env::var("TR_STARTS")
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|t| t.lines().map(|l| json_str_array(l, "\"moves\"")).filter(|v| v.len() >= 12).collect())
        .unwrap_or_default();
    // TR_DG_DEPTH2=<d>: asymmetric games; one colour (alternating by game) searches at depth d instead.
    let depth2: Option<u32> = std::env::var("TR_DG_DEPTH2").ok().and_then(|v| v.parse().ok());
    let mut done = 0;
    while done < games {
        let mut b = Board::new(gt);
        let mut nrand = 4 + (rng.next() % 5) as u32;
        let mut moves: Vec<String> = Vec::new();
        let mut scores: Vec<String> = Vec::new();
        let mut ply = 0u32;
        let mut start = 0u32;
        if !starts.is_empty() {
            let g = &starts[(rng.next() % starts.len() as u64) as usize];
            let maxp = (g.len() - 4).min(40);
            let n = 8 + (rng.next() % (maxp as u64 - 7)) as usize;
            let mut ok = true;
            for mv in g.iter().take(n) {
                match b.parse_move(mv) {
                    Ok(m) => {
                        moves.push(b.move_to_string(m));
                        b.make(m);
                        ply += 1;
                    }
                    Err(_) => {
                        ok = false;
                        break;
                    }
                }
                if b.result() != 0 {
                    ok = false;
                    break;
                }
            }
            if !ok {
                continue;
            }
            // one random ply after the prefix for diversity
            nrand = ply + (rng.next() % 2) as u32;
            start = ply;
        }
        done += 1;
        while b.result() == 0 && b.repetitions() < 2 && ply < 300 {
            legal.clear();
            b.legal_moves(&mut legal);
            if legal.is_empty() {
                break;
            }
            let m = if ply < nrand {
                legal[(rng.next() % legal.len() as u64) as usize]
            } else {
                let d = match depth2 {
                    Some(d2) if (ply as usize + done) % 2 == 0 => d2,
                    _ => depth,
                };
                let res = searcher.search(&b, Limits { time: None, depth: Some(d) }, 1, |_, _, _| {});
                scores.push(format!("[{},{}]", ply, res.score));
                res.best
            };
            moves.push(b.move_to_string(m));
            b.make(m);
            ply += 1;
        }
        let r = match b.result() {
            1 => "1",
            2 => "0",
            _ => "0.5",
        };
        let mv_json: Vec<String> = moves.iter().map(|m| format!("\"{}\"", m.replace('\\', "\\\\"))).collect();
        let _ = writeln!(f, "{{\"gt\":\"Base+MLP\",\"start\":{},\"moves\":[{}],\"scores\":[{}],\"result\":{}}}", start, mv_json.join(","), scores.join(","), r);
    }
}

/// `thoraxrex dumpfeat <in.jsonl> <out.csv>`: replay games and write one row per position:
/// side-to-move features (NF), opponent features (NF), search score for side to move (or empty), result for side to move.
pub fn dumpfeat(args: &[String]) {
    use std::io::BufRead;
    let inp = args.get(0).expect("input");
    let out = args.get(1).expect("output");
    let extra = args.get(2).map(|s| s == "extra").unwrap_or(false);
    let r = std::io::BufReader::new(std::fs::File::open(inp).expect("open input"));
    let mut f = std::io::BufWriter::new(std::fs::File::create(out).expect("create output"));
    let gt = GameType::parse("Base+MLP").unwrap();
    let mut ngames = 0;
    for line in r.lines() {
        let Ok(line) = line else { continue };
        let moves = json_str_array(&line, "\"moves\"");
        let scores = json_pairs(&line, "\"scores\"");
        let result: f64 = json_num(&line, "\"result\"").unwrap_or(0.5);
        let start = json_num(&line, "\"start\"").unwrap_or(0.0) as usize;
        let mut b = Board::new(gt);
        let mut ok = true;
        for (ply, mv) in moves.iter().enumerate() {
            if b.result() != 0 {
                break;
            }
            if ply >= 4.max(start) {
                let fb = crate::eval::features_both(&mut b);
                let me = b.side() as usize;
                let sc = scores.iter().find(|p| p.0 == ply as i64).map(|p| p.1.to_string()).unwrap_or_default();
                let res = if me == 0 { result } else { 1.0 - result };
                let mut row = String::with_capacity(1024);
                for v in fb[me].iter().chain(fb[1 - me].iter()) {
                    row.push_str(&v.to_string());
                    row.push(',');
                }
                if extra {
                    let mut ex = [0f32; crate::eval::EXTRA];
                    crate::eval::extra_inputs(&b, &mut ex);
                    for v in ex.iter() {
                        row.push_str(if *v == 0.0 { "0" } else if *v == 1.0 { "1" } else { "" });
                        if *v != 0.0 && *v != 1.0 {
                            row.push_str(&format!("{}", v));
                        }
                        row.push(',');
                    }
                }
                row.push_str(&sc);
                row.push(',');
                row.push_str(&format!("{}", res));
                let _ = writeln!(f, "{}", row);
            }
            match b.parse_move(mv) {
                Ok(m) => {
                    b.make(m);
                }
                Err(_) => {
                    ok = false;
                    break;
                }
            }
        }
        if ok {
            ngames += 1;
        }
    }
    eprintln!("dumpfeat: {} games", ngames);
}

fn after_key<'a>(s: &'a str, key: &str, open: char) -> Option<&'a str> {
    let i = s.find(key)?;
    let rest = &s[i + key.len()..];
    let j = rest.find(open)?;
    Some(&rest[j + 1..])
}

pub fn json_str_array(s: &str, key: &str) -> Vec<String> {
    let Some(rest) = after_key(s, key, '[') else { return vec![] };
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_str = false;
    let mut esc = false;
    for ch in rest.chars() {
        if in_str {
            if esc {
                cur.push(ch);
                esc = false;
            } else if ch == '\\' {
                esc = true;
            } else if ch == '"' {
                in_str = false;
                out.push(std::mem::take(&mut cur));
            } else {
                cur.push(ch);
            }
        } else if ch == '"' {
            in_str = true;
        } else if ch == ']' {
            break;
        }
    }
    out
}

fn json_pairs(s: &str, key: &str) -> Vec<(i64, i64)> {
    let Some(rest) = after_key(s, key, '[') else { return vec![] };
    let rest = rest.trim_start();
    if rest.starts_with(']') {
        return vec![];
    }
    let end = rest.find("]]").map(|e| e + 1).unwrap_or(0);
    let body = &rest[..end];
    body.split("],[")
        .filter_map(|p| {
            let p = p.trim_matches(|c| c == '[' || c == ']');
            let mut it = p.split(',');
            Some((it.next()?.trim().parse().ok()?, it.next()?.trim().parse().ok()?))
        })
        .collect()
}

fn json_num(s: &str, key: &str) -> Option<f64> {
    let rest = after_key(s, key, ':')?;
    let end = rest.find(|c: char| c == ',' || c == '}').unwrap_or(rest.len());
    rest[..end].trim().parse().ok()
}

/// String value of `key` (simple, no escapes inside), e.g. "w":"Formi".
pub fn json_str(s: &str, key: &str) -> Option<String> {
    let i = s.find(key)? + key.len();
    let rest = &s[i..];
    let a = rest.find('"')? + 1;
    let b = rest[a..].find('"')? + a;
    Some(rest[a..b].to_string())
}

/// `thoraxrex relabel <in.jsonl> <out.jsonl> <ms> <shard> <nshards> [step]`: replay real games
/// ("moves" array, "result" 1/0/0.5 for White) and label every `step`-th position (plies 6..80)
/// with a timed single-thread search. Output lines are dumpfeat-compatible.
pub fn relabel(args: &[String]) {
    use std::io::BufRead;
    let inp = &args[0];
    let out = &args[1];
    let ms: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(250);
    let shard: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(0);
    let nsh: usize = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(1);
    let step: usize = args.get(5).and_then(|s| s.parse().ok()).unwrap_or(2);
    let gt = GameType::parse("Base+MLP").unwrap();
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(out).expect("open output");
    let mut searcher = Searcher::new(32);
    let r = std::io::BufReader::new(std::fs::File::open(inp).expect("open input"));
    for (gi, line) in r.lines().enumerate() {
        let Ok(line) = line else { continue };
        if gi % nsh != shard {
            continue;
        }
        let moves = json_str_array(&line, "\"moves\"");
        let result = json_num(&line, "\"result\"").unwrap_or(0.5);
        let mut b = Board::new(gt);
        let mut scores = Vec::new();
        let mut done = Vec::new();
        let off = gi % step;
        for (ply, mv) in moves.iter().enumerate() {
            if b.result() != 0 || ply > 80 {
                break;
            }
            if ply >= 6 && (ply + off) % step == 0 {
                let res = searcher.search(&b, Limits { time: Some(std::time::Duration::from_millis(ms)), depth: None }, 1, |_, _, _| {});
                scores.push(format!("[{},{}]", ply, res.score));
            }
            match b.parse_move(mv) {
                Ok(m) => {
                    done.push(b.move_to_string(m));
                    b.make(m);
                }
                Err(_) => break,
            }
        }
        let mv_json: Vec<String> = done.iter().map(|m| format!("\"{}\"", m.replace('\\', "\\\\"))).collect();
        let _ = writeln!(f, "{{\"gt\":\"Base+MLP\",\"start\":0,\"moves\":[{}],\"scores\":[{}],\"result\":{}}}", mv_json.join(","), scores.join(","), result);
    }
}

/// `thoraxrex neteval <games.jsonl> <n>`: print the raw net output for the first n dumpfeat positions.
pub fn neteval(args: &[String]) {
    use std::io::BufRead;
    let n: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(20);
    let net = crate::eval::net().expect("no net");
    let gt = GameType::parse("Base+MLP").unwrap();
    let r = std::io::BufReader::new(std::fs::File::open(&args[0]).expect("open"));
    let mut k = 0;
    for line in r.lines() {
        let Ok(line) = line else { continue };
        let moves = json_str_array(&line, "\"moves\"");
        let start = json_num(&line, "\"start\"").unwrap_or(0.0) as usize;
        let mut b = Board::new(gt);
        for (ply, mv) in moves.iter().enumerate() {
            if b.result() != 0 { break; }
            if ply >= 4.max(start) {
                let fb = crate::eval::features_both(&mut b);
                let me = b.side() as usize;
                let mut ex = [0f32; crate::eval::EXTRA];
                crate::eval::extra_inputs(&b, &mut ex);
                println!("{}", net.eval(&fb[me], &fb[1 - me], &ex));
                k += 1;
                if k >= n { return; }
            }
            match b.parse_move(mv) { Ok(m) => { b.make(m); } Err(_) => break }
        }
    }
}

/// `thoraxrex danger <games.jsonl> <depth> <shard> <nshards> <back>`: for each game, search the
/// positions 2..=back plies before the end at fixed depth; print "game ply total score(side to move)".
pub fn danger(args: &[String]) {
    use std::io::BufRead;
    let depth: u32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(3);
    let shard: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    let nsh: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1);
    let back: usize = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(16);
    let gt = GameType::parse("Base+MLP").unwrap();
    let mut searcher = Searcher::new(32);
    let r = std::io::BufReader::new(std::fs::File::open(&args[0]).expect("open"));
    for (gi, line) in r.lines().enumerate() {
        let Ok(line) = line else { continue };
        if gi % nsh != shard {
            continue;
        }
        let moves = json_str_array(&line, "\"moves\"");
        let total = moves.len();
        let mut b = Board::new(gt);
        searcher.clear();
        for (ply, mv) in moves.iter().enumerate() {
            if b.result() != 0 {
                break;
            }
            let left = total - ply;
            if ply >= 8 && left >= 2 && left <= back {
                let lim = if depth >= 50 { Limits { time: Some(std::time::Duration::from_millis(depth as u64)), depth: None } } else { Limits { time: None, depth: Some(depth) } };
                let res = searcher.search(&b, lim, 1, |_, _, _| {});
                println!("{} {} {} {}", gi, ply, total, res.score);
            }
            match b.parse_move(mv) {
                Ok(m) => { b.make(m); }
                Err(_) => break,
            }
        }
    }
}

/// `thoraxrex bookcheck <in> <out> <ms> <threads> <shard> <nshards>`: for each book line
/// `key|w|move|n|p|prefix`, search the position (engine's own score and move) and the position
/// after the book move; append `key|w|move|n|p|own_score|own_move|book_score` (scores for the mover).
pub fn bookcheck(args: &[String]) {
    use std::io::BufRead;
    let ms: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(2000);
    let threads: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1);
    let shard: u64 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(0);
    let nsh: u64 = args.get(5).and_then(|s| s.parse().ok()).unwrap_or(1);
    let gt = GameType::parse("Base+MLP").unwrap();
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&args[1]).expect("open output");
    let mut searcher = Searcher::new(64);
    let lim = || Limits { time: Some(std::time::Duration::from_millis(ms)), depth: None };
    let r = std::io::BufReader::new(std::fs::File::open(&args[0]).expect("open input"));
    let mut last: Option<(String, i32, String)> = None;
    for line in r.lines() {
        let Ok(line) = line else { continue };
        let p: Vec<&str> = line.split('|').collect();
        if p.len() < 6 {
            continue;
        }
        let key = u64::from_str_radix(p[0], 16).unwrap_or(0);
        if key % nsh != shard {
            continue;
        }
        let mut b = Board::new(gt);
        let mut ok = true;
        for mv in p[5].split(';').filter(|s| !s.is_empty()) {
            match b.parse_move(mv) {
                Ok(m) => { b.make(m); }
                Err(_) => { ok = false; break; }
            }
        }
        if !ok {
            continue;
        }
        let (own, own_mv) = match &last {
            Some((k, s, m)) if k == p[0] => (*s, m.clone()),
            _ => {
                searcher.clear();
                let res = searcher.search(&b, lim(), threads, |_, _, _| {});
                (res.score, b.move_to_string(res.best))
            }
        };
        last = Some((p[0].to_string(), own, own_mv.clone()));
        let Ok(m) = b.parse_move(p[2]) else { continue };
        b.make(m);
        searcher.clear();
        let res = searcher.search(&b, lim(), threads, |_, _, _| {});
        let _ = writeln!(f, "{}|{}|{}|{}|{}|{}|{}|{}", p[0], p[1], p[2], p[3], p[4], own, own_mv, -res.score);
    }
}
