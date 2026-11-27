use crate::board::*;
use crate::search::{report_score, Limits, Searcher};
use std::time::Duration;

pub const NAME: &str = "FoulBrood";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct Engine {
    /// The set-up position this game started from, when it did not start from the empty board.
    pub root: Option<String>,
    pub board: Board,
    pub moves: Vec<String>,
    pub undos: Vec<Undo>,
    pub started: bool,
    pub searcher: Searcher,
    pub report: bool,
    pub threads: usize,
    pub hash_mb: usize,
    pub use_book: bool,
}

impl Engine {
    pub fn new() -> Engine {
        // Uses what the machine has: one search thread per core (up to TR_MAXTHREADS, default 8)
        // and a position memory sized from the computer's RAM (see auto_hash_mb). Viewers can override both.
        let cap = std::env::var("TR_MAXTHREADS").ok().and_then(|v| v.parse::<usize>().ok()).filter(|n| (1..=64).contains(n)).unwrap_or(8);
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).min(cap);
        let hash_mb = auto_hash_mb();
        Engine {
            root: None,
            board: Board::new(GameType::default()),
            moves: Vec::new(),
            undos: Vec::new(),
            started: false,
            searcher: Searcher::new(hash_mb),
            report: true,
            threads,
            hash_mb,
            use_book: true,
        }
    }

    pub fn info() -> Vec<String> {
        // Shown in the viewer's engine list: "FoulBrood Book" with an opening book, "FoulBrood" without.
        let tag = if crate::book::has_book() { " Book" } else { "" };
        vec![format!("id {}{}", NAME, tag), "Mosquito;Ladybug;Pillbug;FoulBroodSetup".into()]
    }

    fn state_string(&self) -> &'static str {
        if !self.started {
            return "NotStarted";
        }
        match self.board.result() {
            1 => "WhiteWins",
            2 => "BlackWins",
            3 => "Draw",
            _ => {
                if self.board.repetitions() >= 2 {
                    "Draw"
                } else if self.board.ply == 0 {
                    "NotStarted"
                } else {
                    "InProgress"
                }
            }
        }
    }

    pub fn game_over(&self) -> bool {
        // Repetition draws are reported in the GameString, but play is still accepted so a
        // viewer with slightly different repetition rules never gets stuck.
        self.started && self.board.result() != 0
    }

    pub fn game_string(&self) -> String {
        let side = if self.board.side() == 0 { "White" } else { "Black" };
        let turn = self.board.ply / 2 + 1;
        let kind = self.root.clone().unwrap_or_else(|| self.board.gt.to_string());
        let mut s = format!("{};{};{}[{}]", kind, self.state_string(), side, turn);
        for m in &self.moves {
            s.push(';');
            s.push_str(m);
        }
        s
    }

    pub fn execute(&mut self, line: &str) -> Vec<String> {
        let line = line.trim();
        let (cmd, args) = match line.find(' ') {
            Some(i) => (&line[..i], line[i + 1..].trim()),
            None => (line, ""),
        };
        match cmd {
            "" => vec![],
            "info" => Self::info(),
            "newgame" => self.newgame(args),
            "play" => self.play(args),
            "pass" => self.play("pass"),
            "validmoves" => self.validmoves(),
            "bestmove" => self.bestmove(args),
            "undo" => self.undo(args),
            "options" => self.options(args),
            "features" => {
                let f = crate::eval::features(&mut self.board);
                vec![f.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",")]
            }
            "featboth" => {
                let f = crate::eval::features_both(&mut self.board);
                f.iter().map(|r| r.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",")).collect()
            }
            "key" => vec![format!("{:016x}", self.board.book_key())],
            // Symmetry test hooks: `symkey <t>` and `symmove <t> <MoveString>`.
            "symkey" => vec![format!("{:016x}", crate::book::sym_key(&self.board, args.trim().parse().unwrap_or(0)))],
            "symmove" => {
                let mut it = args.trim().splitn(2, ' ');
                let t: usize = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
                vec![crate::book::sym_move(it.next().unwrap_or(""), t)]
            }
            "policy" => {
                let Some(pn) = crate::policy::net() else { return vec!["err no policy net".into()] };
                let mut mv = Vec::new();
                self.board.legal_moves(&mut mv);
                let lg = pn.logits(&self.board, &mv);
                let mx = lg.iter().cloned().fold(f32::MIN, f32::max);
                let z: f32 = lg.iter().map(|v| (v - mx).exp()).sum();
                let mut idx: Vec<usize> = (0..mv.len()).collect();
                idx.sort_by(|a, b| lg[*b].partial_cmp(&lg[*a]).unwrap());
                idx.iter().take(8).map(|&i| format!("{} {:.3}", self.board.move_to_string(mv[i]), (lg[i] - mx).exp() / z)).collect()
            }
            "evaluate" => vec![crate::eval::evaluate(&mut self.board).to_string()],
            "perft" => match args.parse::<u32>() {
                Ok(d) => vec![self.board.perft(d).to_string()],
                Err(_) => vec!["err perft requires a depth.".into()],
            },
            _ => vec![format!("err Invalid command '{}'.", cmd)],
        }
    }

    fn setup_board(text: &str) -> Result<Board, String> {
        let f: Vec<&str> = text.split('~').collect();
        if f.len() != 5 {
            return Err("Invalid setup position.".into());
        }
        let gt = GameType::parse(f[0]).ok_or_else(|| format!("Invalid GameTypeString '{}'.", f[0]))?;
        let side = match f[1] {
            "0" => 0u8,
            "1" => 1u8,
            _ => return Err("Choose White or Black to move.".into()),
        };
        let turn: u32 = f[2].parse().map_err(|_| "Invalid turn number.".to_string())?;
        let coord = |x: &str| x.parse::<i32>().ok().filter(|v| v.abs() <= 100).ok_or_else(|| "Invalid setup coordinate.".to_string());
        let mut pieces = Vec::new();
        if !f[3].is_empty() {
            for entry in f[3].split(',') {
                let p: Vec<&str> = entry.split(':').collect();
                if p.len() != 4 {
                    return Err("Invalid setup piece.".into());
                }
                let piece = parse_piece(p[0]).ok_or_else(|| format!("Unknown piece '{}'.", p[0]))?;
                let level: usize = p[3].parse().map_err(|_| "Invalid stack level.".to_string())?;
                pieces.push((piece, coord(p[1])?, coord(p[2])?, level));
            }
        }
        let last = if f[4].is_empty() {
            None
        } else {
            let p: Vec<&str> = f[4].split(':').collect();
            if p.len() != 3 {
                return Err("Invalid last moved piece.".into());
            }
            Some(parse_piece(p[0]).ok_or_else(|| format!("Unknown piece '{}'.", p[0]))?)
        };
        Board::from_setup(gt, side, turn, &pieces, last)
    }

    fn newgame(&mut self, args: &str) -> Vec<String> {
        let fields: Vec<&str> = if args.is_empty() { vec!["Base"] } else { args.split(';').collect() };
        // FoulBrood analysis root: variant~side~turn~piece:q:r:level,...~piece:from_q:from_r (a set-up position).
        let root = fields[0].trim();
        let board = if root.contains('~') {
            match Self::setup_board(root) {
                Ok(b) => b,
                Err(e) => return vec![format!("err {}", e)],
            }
        } else {
            match GameType::parse(root) {
                Some(g) => Board::new(g),
                None => return vec![format!("err Invalid GameTypeString '{}'.", fields[0])],
            }
        };
        let mut fresh = Engine {
            root: if root.contains('~') { Some(root.to_string()) } else { None },
            board,
            moves: Vec::new(),
            undos: Vec::new(),
            started: true,
            searcher: std::mem::replace(&mut self.searcher, Searcher::new(1)),
            report: self.report,
            threads: self.threads,
            hash_mb: self.hash_mb,
            use_book: self.use_book,
        };
        if fields.len() > 1 {
            if fields.len() < 3 {
                self.searcher = fresh.searcher;
                return vec!["err GameString must contain type, state and turn.".into()];
            }
            for mv in &fields[3..] {
                let r = fresh.play(mv.trim());
                if let Some(e) = r.first().filter(|s| s.starts_with("err") || s.starts_with("invalidmove")) {
                    let e = e.clone();
                    self.searcher = fresh.searcher;
                    return vec![format!("err Invalid GameString: {}", e)];
                }
            }
        }
        fresh.searcher.clear();
        *self = fresh;
        vec![self.game_string()]
    }

    fn play(&mut self, args: &str) -> Vec<String> {
        if !self.started {
            return vec!["err No game in progress. Try 'newgame' to start a new game.".into()];
        }
        if self.game_over() {
            return vec!["err The game is over.".into()];
        }
        match self.board.parse_move(args) {
            Ok(m) => {
                let text = self.board.move_to_string(m);
                let u = self.board.make(m);
                self.undos.push(u);
                self.moves.push(text);
                vec![self.game_string()]
            }
            Err(e) => vec![format!("invalidmove {}", e)],
        }
    }

    fn validmoves(&mut self) -> Vec<String> {
        if !self.started {
            return vec!["err No game in progress.".into()];
        }
        if self.game_over() {
            return vec![String::new()];
        }
        let mut v = Vec::new();
        self.board.legal_moves(&mut v);
        let strs: Vec<String> = v.iter().map(|&m| self.board.move_to_string(m)).collect();
        vec![strs.join(";")]
    }

    fn undo(&mut self, args: &str) -> Vec<String> {
        let n = if args.is_empty() {
            1
        } else {
            match args.parse::<usize>() {
                Ok(n) if n >= 1 => n,
                _ => return vec!["err undo requires a positive integer.".into()],
            }
        };
        if n > self.undos.len() {
            return vec![format!("err Cannot undo {} move(s).", n)];
        }
        for _ in 0..n {
            let u = self.undos.pop().unwrap();
            self.board.unmake(u);
            self.moves.pop();
        }
        vec![self.game_string()]
    }

    fn options(&mut self, args: &str) -> Vec<String> {
        let opt_line = |e: &Engine, name: &str| -> Option<String> {
            match name {
                "ReportFoulBroodScores" => Some(format!(
                    "ReportFoulBroodScores;bool;{};True",
                    if e.report { "True" } else { "False" }
                )),
                "Threads" => Some(format!("Threads;int;{};{};1;64", e.threads, e.threads)),
                "HashMB" => Some(format!("HashMB;int;{};{};1;4096", e.hash_mb, auto_hash_mb())),
                "UseBook" => Some(format!("UseBook;bool;{};True", if e.use_book { "True" } else { "False" })),
                _ => None,
            }
        };
        let parts: Vec<&str> = args.split_whitespace().collect();
        match parts.as_slice() {
            [] => ["ReportFoulBroodScores", "Threads", "HashMB", "UseBook"]
                .iter()
                .filter_map(|n| opt_line(self, n))
                .collect(),
            ["get", name] => match opt_line(self, name) {
                Some(l) => vec![l],
                None => vec![format!("err Unknown option '{}'.", name)],
            },
            ["set", name, value] => {
                match *name {
                    "ReportFoulBroodScores" => match *value {
                        "True" => self.report = true,
                        "False" => self.report = false,
                        _ => return vec!["err ReportFoulBroodScores must be True or False.".into()],
                    },
                    "UseBook" => match *value {
                        "True" => self.use_book = true,
                        "False" => self.use_book = false,
                        _ => return vec!["err UseBook must be True or False.".into()],
                    },
                    "Threads" => match value.parse::<usize>() {
                        Ok(n) if (1..=64).contains(&n) => self.threads = n,
                        _ => return vec!["err Threads must be 1 to 64.".into()],
                    },
                    "HashMB" => match value.parse::<usize>() {
                        Ok(n) if (1..=4096).contains(&n) => {
                            self.hash_mb = n;
                            self.searcher = Searcher::new(n);
                        }
                        _ => return vec!["err HashMB must be 1 to 4096.".into()],
                    },
                    _ => return vec![format!("err Unknown option '{}'.", name)],
                }
                vec![opt_line(self, name).unwrap()]
            }
            _ => vec!["err Invalid options command.".into()],
        }
    }

    fn bestmove(&mut self, args: &str) -> Vec<String> {
        if !self.started {
            return vec!["err No game in progress.".into()];
        }
        if self.game_over() {
            return vec!["err The game is over.".into()];
        }
        if self.use_book {
            if let Some(m) = crate::book::probe(&mut self.board) {
                let text = self.board.move_to_string(m);
                if self.report {
                    eprintln!("FoulBroodScore v1;depth=1;score=0;move={}", text);
                    eprintln!("FoulBroodScore v2;kind=final;status=complete;depth=1;score=0;move={};evalmove={}", text, text);
                }
                return vec![text];
            }
        }
        let parts: Vec<&str> = args.split_whitespace().collect();
        let limits = match parts.as_slice() {
            ["time", t] => match parse_time(t) {
                Some(d) => Limits { time: Some(d), depth: None },
                None => return vec!["err Invalid time; use hh:mm:ss.".into()],
            },
            ["depth", d] => match d.parse::<u32>() {
                Ok(d) => Limits { time: None, depth: Some(d.clamp(1, 60)) },
                Err(_) => return vec!["err Invalid depth.".into()],
            },
            _ => return vec!["err bestmove requires 'time hh:mm:ss' or 'depth N'.".into()],
        };
        let report = self.report;
        let board = self.board.clone();
        let res = self.searcher.search(&board, limits, self.threads, |depth, score, mv| {
            let score = report_score(score);
            if report {
                let text = board.move_to_string(mv);
                eprintln!("FoulBroodScore v1;depth={};score={};move={}", depth, score, text);
                eprintln!("FoulBroodScore v2;kind=iteration;depth={};score={};move={}", depth, score, text);
            }
        });
        let text = self.board.move_to_string(res.best);
        eprintln!("info depth {} score {} nodes {} time_ms {} qnodes {}", res.depth, res.score, res.nodes, res.ms, crate::search::QN.swap(0, std::sync::atomic::Ordering::Relaxed));
        let fscore = report_score(res.score);
        if report && res.depth > 0 {
            eprintln!("FoulBroodScore v1;depth={};score={};move={}", res.depth, fscore, text);
            eprintln!(
                "FoulBroodScore v2;kind=final;status=complete;depth={};score={};move={};evalmove={}",
                res.depth, fscore, text, text
            );
        }
        vec![text]
    }
}

fn parse_time(s: &str) -> Option<Duration> {
    let p: Vec<&str> = s.split(':').collect();
    if p.len() != 3 {
        return None;
    }
    let h: u64 = p[0].parse().ok()?;
    let m: u64 = p[1].parse().ok()?;
    let sec: f64 = p[2].parse().ok()?;
    if m >= 60 || sec < 0.0 || sec >= 60.0 {
        return None;
    }
    Some(Duration::from_secs(h * 3600 + m * 60) + Duration::from_secs_f64(sec))
}

/// Installed memory in bytes, if the system will say (macOS, Linux, Windows).
fn total_ram() -> Option<u64> {
    #[cfg(target_os = "macos")]
    {
        let out = std::process::Command::new("/usr/sbin/sysctl").args(["-n", "hw.memsize"]).output().ok()?;
        return String::from_utf8_lossy(&out.stdout).trim().parse().ok();
    }
    #[cfg(target_os = "linux")]
    {
        let text = std::fs::read_to_string("/proc/meminfo").ok()?;
        let kb: u64 = text.lines().find(|l| l.starts_with("MemTotal:"))?.split_whitespace().nth(1)?.parse().ok()?;
        return Some(kb * 1024);
    }
    #[cfg(windows)]
    {
        #[repr(C)]
        struct MemoryStatusEx { len: u32, load: u32, total_phys: u64, avail_phys: u64, total_page: u64, avail_page: u64, total_virt: u64, avail_virt: u64, avail_ext: u64 }
        #[link(name = "kernel32")]
        extern "system" { fn GlobalMemoryStatusEx(buf: *mut MemoryStatusEx) -> i32; }
        let mut m = MemoryStatusEx { len: std::mem::size_of::<MemoryStatusEx>() as u32, load: 0, total_phys: 0, avail_phys: 0, total_page: 0, avail_page: 0, total_virt: 0, avail_virt: 0, avail_ext: 0 };
        return if unsafe { GlobalMemoryStatusEx(&mut m) } != 0 { Some(m.total_phys) } else { None };
    }
    #[allow(unreachable_code)]
    None
}

/// Position memory (hash) in MB: TR_HASHMB if set, else about 1/32 of RAM as a power of two,
/// between 64 MB and 1024 MB (64 MB when the RAM can't be read). A 24 GB Mac gets 512 MB.
pub fn auto_hash_mb() -> usize {
    if let Some(n) = std::env::var("TR_HASHMB").ok().and_then(|v| v.parse::<usize>().ok()).filter(|n| (1..=4096).contains(n)) {
        return n;
    }
    let mb = total_ram().map(|b| (b / 32 / (1024 * 1024)) as usize).unwrap_or(64).clamp(64, 1024);
    1 << (usize::BITS - 1 - mb.leading_zeros()) // round down to a power of two
}
