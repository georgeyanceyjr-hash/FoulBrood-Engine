//! Iterative-deepening PVS alpha-beta with a shared lockless transposition table
//! and Lazy SMP helper threads.

use crate::board::*;
use crate::eval::evaluate;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const MATE: i32 = 30000;
pub const INF: i32 = 32000;
const CONTEMPT: i32 = 25;
const LMR_A: f64 = 1.2;
const LMR_B: f64 = 1.8;
pub static QN: AtomicU64 = AtomicU64::new(0);
fn lmp_limit(depth: i32) -> usize {
    use std::sync::OnceLock;
    static L: OnceLock<[usize; 4]> = OnceLock::new();
    let l = L.get_or_init(|| {
        let base: usize = std::env::var("TR_LMP").ok().and_then(|v| v.parse().ok()).unwrap_or(1000);
        [0, base, base * 2, base * 4]
    });
    l[depth.clamp(1, 3) as usize]
}
fn qmax() -> u32 {
    use std::sync::OnceLock;
    static Q: OnceLock<u32> = OnceLock::new();
    *Q.get_or_init(|| std::env::var("TR_QMAX").ok().and_then(|v| v.parse().ok()).unwrap_or(0))
}
const MAX_PLY: usize = 128;

static LMR: std::sync::LazyLock<Box<[[u8; 256]; 64]>> = std::sync::LazyLock::new(|| {
    let mut t = Box::new([[0u8; 256]; 64]);
    for d in 1..64 {
        for i in 1..256 {
            t[d][i] = (LMR_A + (d as f64).ln() * (i as f64).ln() / LMR_B) as u8;
        }
    }
    t
});

#[derive(Clone, Copy)]
pub struct Limits {
    pub time: Option<Duration>,
    pub depth: Option<u32>,
}

pub struct SearchResult {
    pub best: Move,
    pub score: i32,
    pub depth: u32,
    pub nodes: u64,
    pub ms: u64,
}

// ---------------------------------------------------------------- TT
pub struct TT {
    data: Vec<AtomicU64>, // pairs: [key ^ val, val]
    mask: usize,
}

const EXACT: u64 = 1;
const LOWER: u64 = 2;
const UPPER: u64 = 3;

impl TT {
    pub fn new(mb: usize) -> TT {
        let entries = (mb * 1024 * 1024 / 16).next_power_of_two() / 2;
        let entries = entries.max(1024);
        let mut data = Vec::with_capacity(entries * 2);
        for _ in 0..entries * 2 {
            data.push(AtomicU64::new(0));
        }
        TT { data, mask: entries - 1 }
    }
    pub fn clear(&self) {
        for a in &self.data {
            a.store(0, Ordering::Relaxed);
        }
    }
    #[inline(always)]
    fn pack(mv: Move, score: i32, depth: u32, bound: u64) -> u64 {
        (mv.0 as u64) | (((score as i16) as u16 as u64) << 32) | ((depth.min(255) as u64) << 48) | (bound << 56)
    }
    #[inline(always)]
    pub fn probe(&self, key: u64) -> Option<(Move, i32, u32, u64)> {
        let i = (key as usize & self.mask) * 2;
        let k = self.data[i].load(Ordering::Relaxed);
        let v = self.data[i + 1].load(Ordering::Relaxed);
        if k ^ v != key || v == 0 {
            return None;
        }
        Some((
            Move(v as u32),
            ((v >> 32) as u16 as i16) as i32,
            ((v >> 48) & 255) as u32,
            (v >> 56) & 3,
        ))
    }
    #[inline(always)]
    pub fn store(&self, key: u64, mv: Move, score: i32, depth: u32, bound: u64) {
        let i = (key as usize & self.mask) * 2;
        let old_v = self.data[i + 1].load(Ordering::Relaxed);
        let old_k = self.data[i].load(Ordering::Relaxed) ^ old_v;
        let old_depth = ((old_v >> 48) & 255) as u32;
        let mut mv = mv;
        if old_k == key {
            if depth + 2 < old_depth && bound != EXACT {
                return;
            }
            if mv == Move::NULL {
                mv = Move(old_v as u32);
            }
        }
        let v = Self::pack(mv, score, depth, bound);
        self.data[i].store(key ^ v, Ordering::Relaxed);
        self.data[i + 1].store(v, Ordering::Relaxed);
    }
}

// ---------------------------------------------------------------- Searcher
pub struct Searcher {
    pub tt: Arc<TT>,
}

struct Shared {
    tt: Arc<TT>,
    stop: AtomicBool,
    nodes: AtomicU64,
    helper_best: AtomicU64, // depth<<48 | (score as u16)<<32 | move
    start: Instant,
    hard: Option<Duration>,
}

struct Worker<'a> {
    b: Board,
    sh: &'a Shared,
    nodes: u64,
    killers: [[Move; 2]; MAX_PLY],
    history: Box<[[i32; 1024]; 28]>,
    moves: Vec<Vec<Move>>,
    scores: Vec<Vec<i32>>,
    root_ply: u32,
    pv_root: Move,
    root_best: Move,
    qn: u64,
    salt: u64,
    stopped: bool,
    null_ok: bool,
}

#[inline(always)]
fn score_to_tt(s: i32, ply: usize) -> i32 {
    if s > MATE - 1000 {
        s + ply as i32
    } else if s < -MATE + 1000 {
        s - ply as i32
    } else {
        s
    }
}
#[inline(always)]
fn score_from_tt(s: i32, ply: usize) -> i32 {
    if s > MATE - 1000 {
        s - ply as i32
    } else if s < -MATE + 1000 {
        s + ply as i32
    } else {
        s
    }
}

impl Searcher {
    pub fn new(mb: usize) -> Searcher {
        Searcher { tt: Arc::new(TT::new(mb)) }
    }
    pub fn clear(&mut self) {
        self.tt.clear();
    }

    pub fn search<F: FnMut(u32, i32, Move)>(
        &mut self,
        root: &Board,
        limits: Limits,
        threads: usize,
        mut report: F,
    ) -> SearchResult {
        let start = Instant::now();
        crate::eval::ROOT_COLOR.store(root.side() as u8, std::sync::atomic::Ordering::Relaxed);
        let mut b = root.clone();
        let mut legal = Vec::new();
        b.legal_moves(&mut legal);
        if legal.is_empty() {
            return SearchResult { best: Move::PASS, score: 0, depth: 0, nodes: 0, ms: 0 };
        }
        if legal.len() == 1 {
            return SearchResult { best: legal[0], score: 0, depth: 0, nodes: 0, ms: 0 };
        }
        // Soft/hard time: the viewer gives a per-move budget.
        let (soft, hard) = match limits.time {
            Some(t) => {
                let t = t.saturating_sub(Duration::from_millis(30)).max(Duration::from_millis(20));
                (Some(t.mul_f64(soft_frac())), Some(t))
            }
            None => (None, None),
        };
        let max_depth = limits.depth.unwrap_or(60).min(60);
        let sh = Shared {
            tt: self.tt.clone(),
            stop: AtomicBool::new(false),
            nodes: AtomicU64::new(0),
            helper_best: AtomicU64::new(0),
            start,
            hard,
        };
        let mut best = SearchResult { best: legal[0], score: 0, depth: 0, nodes: 0, ms: 0 };
        std::thread::scope(|scope| {
            let shr = &sh;
            for id in 1..threads.max(1) {
                let rb = root.clone();
                scope.spawn(move || {
                    let mut w = Worker::new(rb, shr);
                    if diverse() {
                        w.salt = (id as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
                    }
                    let mut d = 1 + (id as u32 % 2);
                    while d <= max_depth && !shr.stop.load(Ordering::Relaxed) {
                        let sc = w.root(d, -INF, INF);
                        if !w.stopped && w.pv_root != Move::NULL {
                            let packed = ((d as u64) << 48) | (((sc.clamp(-32000, 32000) + 32768) as u16 as u64) << 32) | w.pv_root.0 as u64;
                            shr.helper_best.fetch_max(packed, Ordering::Relaxed);
                        }
                        d += 1;
                    }
                });
            }
            let mut w = Worker::new(root.clone(), shr);
            let mut prev: i32 = 0;
            let mut last_done = Duration::ZERO;
            let mut prev_it = Duration::ZERO;
            for d in 1..=max_depth {
                let (mut alpha, mut beta) = (-INF, INF);
                let mut window = 40;
                if d >= 4 && prev.abs() < MATE - 1000 {
                    alpha = prev - window;
                    beta = prev + window;
                }
                let mut score;
                loop {
                    score = w.root(d, alpha, beta);
                    if w.stopped {
                        break;
                    }
                    if score <= alpha {
                        alpha = (alpha - window).max(-INF);
                        window *= 3;
                    } else if score >= beta {
                        beta = (beta + window).min(INF);
                        window *= 3;
                    } else {
                        break;
                    }
                    if window > 1500 {
                        alpha = -INF;
                        beta = INF;
                    }
                }
                if w.stopped {
                    // A move that raised alpha in the unfinished iteration was fully searched and
                    // proved better than the window: use it (standard partial-iteration rule).
                    if partial_ok() && w.root_best != Move::NULL && d > 1 {
                        best.best = w.root_best;
                    }
                    break;
                }
                prev = score;
                best = SearchResult { best: w.pv_root, score, depth: d, nodes: 0, ms: 0 };
                report(d, score, w.pv_root);
                if score.abs() > MATE - 1000 && d as i32 > (MATE - score.abs()) + 2 {
                    break;
                }
                let now = start.elapsed();
                if smart_tm() {
                    if let Some(h) = hard {
                        let it = now.saturating_sub(last_done);
                        let ratio = if prev_it.as_micros() > 0 {
                            (it.as_secs_f64() / prev_it.as_secs_f64()).clamp(1.5, 6.0)
                        } else {
                            3.0
                        };
                        let predicted = it.as_secs_f64() * ratio;
                        if now.as_secs_f64() + predicted * 0.8 > h.as_secs_f64() * 0.95 {
                            break;
                        }
                        prev_it = it;
                        last_done = now;
                    }
                } else if let Some(s) = soft {
                    if now > s {
                        break;
                    }
                }
            }
            sh.stop.store(true, Ordering::Relaxed);
        });
        // Use a helper's result if it completed a deeper iteration than the main thread.
        let hb = sh.helper_best.load(Ordering::Relaxed);
        if hb != 0 && use_helpers() {
            let hd = (hb >> 48) as u32;
            let hm = Move(hb as u32);
            if hd > best.depth && legal.contains(&hm) {
                best.best = hm;
                best.depth = hd;
                best.score = ((hb >> 32) & 0xFFFF) as i32 - 32768;
            }
        }
        best.nodes = sh.nodes.load(Ordering::Relaxed);
        best.ms = start.elapsed().as_millis() as u64;
        best
    }
}

impl<'a> Worker<'a> {
    fn new(b: Board, sh: &'a Shared) -> Worker<'a> {
        let root_ply = b.ply;
        Worker {
            b,
            sh,
            nodes: 0,
            killers: [[Move::NULL; 2]; MAX_PLY],
            history: Box::new([[0; 1024]; 28]),
            moves: (0..MAX_PLY).map(|_| Vec::with_capacity(256)).collect(),
            scores: (0..MAX_PLY).map(|_| Vec::with_capacity(256)).collect(),
            root_ply,
            pv_root: Move::NULL,
            root_best: Move::NULL,
            qn: 0,
            salt: 0,
            stopped: false,
            null_ok: true,
        }
    }

    #[inline(always)]
    fn check_stop(&mut self) -> bool {
        if self.stopped {
            return true;
        }
        if self.nodes & 1023 == 0 {
            self.sh.nodes.fetch_add(1024, Ordering::Relaxed);
            QN.fetch_add(std::mem::take(&mut self.qn), Ordering::Relaxed);
            if self.sh.stop.load(Ordering::Relaxed) {
                self.stopped = true;
            } else if let Some(h) = self.sh.hard {
                if self.sh.start.elapsed() >= h {
                    self.sh.stop.store(true, Ordering::Relaxed);
                    self.stopped = true;
                }
            }
        }
        self.stopped
    }

    fn root(&mut self, depth: u32, alpha: i32, beta: i32) -> i32 {
        self.stopped = false;
        self.root_best = Move::NULL;
        if self.sh.stop.load(Ordering::Relaxed) {
            self.stopped = true;
            return 0;
        }
        let s = self.negamax(depth as i32, alpha, beta, 0, true);
        s
    }

    /// Order moves: higher first.
    fn order(&mut self, ply: usize, tt_move: Move) {
        let t = tables();
        let b = &self.b;
        let me = b.side();
        let opp = 1 - me;
        let eq = b.pos[(opp * 14) as usize];
        let mq = b.pos[(me * 14) as usize];
        let moves = &self.moves[ply];
        let scores = &mut self.scores[ply];
        scores.clear();
        let adj = |a: u16, c: u16| -> bool {
            if a == NONE || c == NONE {
                return false;
            }
            let nb = &t.nb[a as usize];
            nb.contains(&c)
        };
        for &m in moves.iter() {
            let mut s: i32;
            if m == tt_move {
                s = 1 << 30;
            } else if m.is_pass() {
                s = 0;
            } else {
                let p = m.piece();
                let to = m.to();
                let from = m.from();
                s = self.history[p as usize][to as usize] / 8;
                if m == self.killers[ply][0] {
                    s += 1 << 20;
                } else if m == self.killers[ply][1] {
                    s += 1 << 19;
                }
                // queen pressure heuristics
                let to_enemy_q = adj(to, eq) && b.hgt[to as usize] == 0;
                let from_enemy_q = adj(from, eq) && b.hgt[from as usize] == 1;
                if to_enemy_q && !from_enemy_q {
                    s += 1 << 22;
                }
                if from_enemy_q && !to_enemy_q {
                    s -= 1 << 18;
                }
                // climbing onto the enemy queen
                if to == eq {
                    s += 1 << 22;
                }
                // relieving own queen
                if p != me * 14 && adj(from, mq) && !adj(to, mq) && b.hgt[from as usize] == 1 {
                    s += 1 << 21;
                }
                if p != me * 14 && adj(to, mq) && !adj(from, mq) && b.hgt[to as usize] == 0 {
                    s -= 1 << 18;
                }
                if m.is_place() {
                    s -= 1 << 16;
                }
                // helper threads: perturb quiet ordering so they explore different subtrees
                if self.salt != 0 {
                    s += ((m.0 as u64 ^ (ply as u64) << 40).wrapping_mul(self.salt) >> 49) as i32;
                }
            }
            scores.push(s);
        }
    }

    #[inline(always)]
    fn pick(&mut self, ply: usize, i: usize) -> Move {
        let scores = &mut self.scores[ply];
        let moves = &mut self.moves[ply];
        let mut bi = i;
        for j in i + 1..moves.len() {
            if scores[j] > scores[bi] {
                bi = j;
            }
        }
        scores.swap(i, bi);
        moves.swap(i, bi);
        moves[i]
    }

    fn negamax(&mut self, depth: i32, mut alpha: i32, beta: i32, ply: usize, pv: bool) -> i32 {
        self.nodes += 1;
        if self.check_stop() {
            return 0;
        }
        // terminal
        match self.b.result() {
            0 => {}
            3 => return 0,
            r => {
                // r: 1 white wins, 2 black wins. Side to move just got moved against.
                let winner = r - 1;
                return if winner == self.b.side() { MATE - ply as i32 } else { -MATE + ply as i32 };
            }
        }
        if ply > 0 && self.b.repetitions() >= 1 {
            return self.draw_score(ply);
        }
        if ply >= MAX_PLY - 2 {
            return evaluate(&mut self.b);
        }
        let mut beta = beta;
        if ply > 0 {
            alpha = alpha.max(-MATE + ply as i32);
            beta = beta.min(MATE - ply as i32 - 1);
            if alpha >= beta {
                return alpha;
            }
        }

        let key = self.b.key();
        let mut tt_move = Move::NULL;
        if let Some((m, s, d, bound)) = self.sh.tt.probe(key) {
            tt_move = m;
            if ply > 0 && d as i32 >= depth && !pv {
                let s = score_from_tt(s, ply);
                match bound {
                    EXACT => return s,
                    LOWER if s >= beta => return s,
                    UPPER if s <= alpha => return s,
                    _ => {}
                }
            }
        }

        if depth <= 0 {
            return self.qsearch(alpha, beta, ply, 0);
        }
        let me = self.b.side();
        let in_danger = self.b.queen_neighbors(me) >= 5 || self.b.queen_neighbors(1 - me) >= 5;
        let se = if !pv && ply > 0 { evaluate(&mut self.b) } else { -INF };
        // reverse futility
        if !pv && ply > 0 && !in_danger && depth <= 3 && se - 110 * depth >= beta && beta.abs() < MATE - 1000 {
            return se;
        }
        // null move
        if !pv && ply > 0 && !in_danger && depth >= 3 && se >= beta && self.b.queen_placed(me)
            && self.b.queen_placed(1 - me) && self.null_ok && beta.abs() < MATE - 1000
        {
            let r = 2 + depth / 4;
            let u = self.b.make(Move::PASS);
            self.null_ok = false;
            let v = -self.negamax(depth - 1 - r, -beta, -beta + 1, ply + 1, false);
            self.null_ok = true;
            self.b.unmake(u);
            if self.stopped {
                return 0;
            }
            if v >= beta {
                return beta;
            }
        }
        let futile = !pv && ply > 0 && !in_danger && depth <= 2 && se + 140 * depth <= alpha;

        // generate
        let mut mv = std::mem::take(&mut self.moves[ply]);
        self.b.legal_moves(&mut mv);
        self.moves[ply] = mv;
        if self.moves[ply].is_empty() {
            return 0;
        }
        // Immediate win check: any move that surrounds the enemy queen.
        let opp = 1 - self.b.side();
        if self.b.queen_neighbors(opp) >= 4 {
            for i in 0..self.moves[ply].len() {
                let m = self.moves[ply][i];
                if m.is_pass() {
                    continue;
                }
                let u = self.b.make(m);
                let r = self.b.result();
                self.b.unmake(u);
                if r != 0 && r != 3 && r - 1 != opp {
                    if ply == 0 {
                        self.pv_root = m;
                    }
                    return MATE - ply as i32 - 1;
                }
            }
        }
        if tt_move != Move::NULL && !self.moves[ply].contains(&tt_move) {
            tt_move = Move::NULL;
        }
        if ply == 0 && tt_move == Move::NULL && self.pv_root != Move::NULL && self.moves[0].contains(&self.pv_root) {
            tt_move = self.pv_root;
        }
        self.order(ply, tt_move);
        if depth >= 2 {
            // refine ordering with a one-ply static look at each child
            let n = self.moves[ply].len();
            for i in 0..n {
                let m = self.moves[ply][i];
                if m == tt_move || m.is_pass() {
                    continue;
                }
                let u = self.b.make(m);
                let r = self.b.result();
                let v = if r != 0 { if r == 3 { 0 } else if r - 1 == self.b.side() { -MATE } else { MATE } } else { -evaluate(&mut self.b) };
                self.b.unmake(u);
                self.scores[ply][i] = self.scores[ply][i] / 4 + v * 64;
            }
        }

        let n = self.moves[ply].len();
        let mut best = -INF;
        let mut best_move = Move::NULL;
        let orig_alpha = alpha;
        for i in 0..n {
            let m = self.pick(ply, i);
            let quiet = self.scores[ply][i] < (1 << 19) && i > 0;
            let lmp_cut = (futile && quiet && best > -MATE + 1000) || !pv && ply > 0 && !in_danger && quiet && depth <= 3 && best > -MATE + 1000
                && i >= lmp_limit(depth);
            let before = self.b.queen_neighbors(opp);
            let before_me = self.b.queen_neighbors(1 - opp);
            if lmp_cut && !qtac() {
                continue;
            }
            let u = self.b.make(m);
            // queen-contact move: changes how many pieces touch either queen
            let qcontact = qtac() && (self.b.queen_neighbors(opp) != before || self.b.queen_neighbors(1 - opp) != before_me);
            if lmp_cut && !qcontact {
                self.b.unmake(u);
                continue;
            }
            let mut ext = 0;
            // extension: this move brought the enemy queen to five neighbours
            if before < 5 && self.b.queen_neighbors(opp) >= 5 && (ply as i32) < 2 * depth + 4 {
                ext = 1;
            }
            let new_depth = depth - 1 + ext;
            let mut score;
            if i == 0 {
                score = -self.negamax(new_depth, -beta, -alpha, ply + 1, pv);
            } else {
                // LMR
                let mut r = 0;
                if depth >= 3 && quiet && i >= 2 {
                    r = LMR[depth.min(63) as usize][i.min(255)] as i32;
                    if pv {
                        r -= 1;
                    }
                    if qcontact {
                        r -= 2;
                    }
                    r = r.clamp(0, depth - 2);
                }
                score = -self.negamax(new_depth - r, -alpha - 1, -alpha, ply + 1, false);
                if score > alpha && r > 0 {
                    score = -self.negamax(new_depth, -alpha - 1, -alpha, ply + 1, false);
                }
                if score > alpha && score < beta {
                    score = -self.negamax(new_depth, -beta, -alpha, ply + 1, true);
                }
            }
            self.b.unmake(u);
            if self.stopped {
                return 0;
            }
            if score > best {
                best = score;
                best_move = m;
                if ply == 0 {
                    self.pv_root = m;
                }
                if score > alpha {
                    alpha = score;
                    if ply == 0 {
                        self.root_best = m;
                    }
                    if score >= beta {
                        if quiet && !m.is_pass() {
                            let k = &mut self.killers[ply];
                            if k[0] != m {
                                k[1] = k[0];
                                k[0] = m;
                            }
                            let h = &mut self.history[m.piece() as usize][m.to() as usize];
                            *h += depth * depth;
                            if *h > 1 << 20 {
                                for row in self.history.iter_mut() {
                                    for x in row.iter_mut() {
                                        *x /= 2;
                                    }
                                }
                            }
                        }
                        break;
                    }
                }
            }
        }
        let bound = if best >= beta {
            LOWER
        } else if best > orig_alpha {
            EXACT
        } else {
            UPPER
        };
        self.sh.tt.store(key, best_move, score_to_tt(best, ply), depth.max(0) as u32, bound);
        best
    }

    /// Draw score with contempt: the engine dislikes draws a little.
    #[inline(always)]
    fn draw_score(&self, ply: usize) -> i32 {
        if ply % 2 == 0 { -CONTEMPT } else { CONTEMPT }
    }

    /// Quiescence over queen-pressure moves (moves that add a neighbour to the
    /// enemy queen or climb onto it), plus immediate win detection.
    fn qsearch(&mut self, mut alpha: i32, beta: i32, ply: usize, qd: u32) -> i32 {
        self.nodes += 1;
        self.qn += 1;
        if self.check_stop() {
            return 0;
        }
        match self.b.result() {
            0 => {}
            3 => return 0,
            r => {
                let winner = r - 1;
                return if winner == self.b.side() { MATE - ply as i32 } else { -MATE + ply as i32 };
            }
        }
        let me = self.b.side();
        let opp = 1 - me;
        let stand = evaluate(&mut self.b);
        let eqn = self.b.queen_neighbors(opp);
        if ply >= MAX_PLY - 2 {
            return stand;
        }
        if eqn < 4 && stand >= beta {
            return stand;
        }
        if qd >= qmax() || !self.b.queen_placed(opp) || eqn < 4 {
            if eqn >= 5 && qd == 0 && qmax() == 0 {
                // still detect an immediate win
            } else {
                return stand;
            }
        }
        let t = tables();
        let eq = self.b.pos[(opp * 14) as usize];
        let mut mv = std::mem::take(&mut self.moves[ply]);
        self.b.legal_moves(&mut mv);
        // keep pressure moves only
        let mut keep: Vec<Move> = Vec::new();
        for &m in &mv {
            if m.is_pass() || m.is_place() {
                continue;
            }
            let to = m.to();
            let from = m.from();
            let adj_to = t.nb[to as usize].contains(&eq) && self.b.hgt[to as usize] == 0;
            let adj_from = t.nb[from as usize].contains(&eq) && self.b.hgt[from as usize] == 1;
            if (adj_to && !adj_from) || to == eq {
                keep.push(m);
            }
        }
        self.moves[ply] = mv;
        if keep.is_empty() {
            return stand;
        }
        let mut best = stand;
        if best > alpha {
            alpha = best;
        }
        if eqn >= 5 {
            // a win may be available: test all pressure moves exactly
            for &m in &keep {
                let u = self.b.make(m);
                let r = self.b.result();
                self.b.unmake(u);
                if r != 0 && r != 3 && r - 1 == me {
                    return MATE - ply as i32 - 1;
                }
            }
        }
        if stand >= beta {
            return stand;
        }
        for &m in &keep {
            let u = self.b.make(m);
            let v = -self.qsearch(-beta, -alpha, ply + 1, qd + 1);
            self.b.unmake(u);
            if self.stopped {
                return 0;
            }
            if v > best {
                best = v;
                if v > alpha {
                    alpha = v;
                    if v >= beta {
                        break;
                    }
                }
            }
        }
        best
    }
}

/// Convert an internal score to the viewer's FoulBrood scale (forced wins beyond +-99000).
pub fn report_score(s: i32) -> i32 {
    if s > MATE - 1000 {
        100000 - (MATE - s)
    } else if s < -MATE + 1000 {
        -100000 + (MATE + s)
    } else {
        s
    }
}

fn soft_frac() -> f64 {
    std::env::var("TR_SOFT").ok().and_then(|v| v.parse::<f64>().ok()).map(|v| v / 100.0).unwrap_or(0.45)
}
fn qtac() -> bool {
    static D: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *D.get_or_init(|| std::env::var("TR_QTAC").map(|v| v != "0").unwrap_or(true))
}
fn partial_ok() -> bool {
    std::env::var("TR_PARTIAL").map(|v| v != "0").unwrap_or(false)
}
fn use_helpers() -> bool {
    std::env::var("TR_HELPERS").map(|v| v != "0").unwrap_or(true)
}
fn diverse() -> bool {
    static D: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *D.get_or_init(|| std::env::var("TR_DIVERSE").map(|v| v != "0").unwrap_or(false))
}
fn smart_tm() -> bool {
    static D: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *D.get_or_init(|| std::env::var("TR_TM").map(|v| v != "0").unwrap_or(false))
}
