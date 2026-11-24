//! Hive rules core: wrapped 32x32 axial grid, stacks, make/unmake, legal move generation
//! for Base + Mosquito / Ladybug / Pillbug.

pub const W: usize = 32;
pub const CELLS: usize = W * W;
pub const NONE: u16 = 0xFFFF;
pub const NOP: u8 = 0xFF;

// Directions in ring order: E, NE, NW, W, SW, SE. Flanks of dir i are i-1 and i+1.
pub const DQ: [i32; 6] = [1, 1, 0, -1, -1, 0];
pub const DR: [i32; 6] = [0, -1, -1, 0, 1, 1];

#[inline(always)]
pub fn cell(q: i32, r: i32) -> u16 {
    ((q & 31) as u16) | (((r & 31) as u16) << 5)
}

pub struct Tables {
    pub nb: [[u16; 6]; CELLS],
    pub zob: [[u64; CELLS]; 28],
    pub zlvl: [[u64; 8]; 28],
    pub zside: u64,
    pub zlast: [u64; 29],
    // repetition hash: by kind (color*8+bug) rather than piece number
    pub rzob: [[u64; CELLS]; 16],
    pub rlvl: [[u64; 8]; 16],
}

fn splitmix(s: &mut u64) -> u64 {
    *s = s.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = *s;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

pub fn tables() -> &'static Tables {
    use std::sync::OnceLock;
    static T: OnceLock<Box<Tables>> = OnceLock::new();
    T.get_or_init(|| {
        let mut t: Box<Tables> = unsafe {
            let layout = std::alloc::Layout::new::<Tables>();
            let p = std::alloc::alloc_zeroed(layout) as *mut Tables;
            Box::from_raw(p)
        };
        for c in 0..CELLS {
            let q = (c & 31) as i32;
            let r = (c >> 5) as i32;
            for d in 0..6 {
                t.nb[c][d] = cell(q + DQ[d], r + DR[d]);
            }
        }
        let mut s = 0x1234_5678_9abc_def0u64;
        for p in 0..28 {
            for c in 0..CELLS {
                t.zob[p][c] = splitmix(&mut s);
            }
            for l in 0..8 {
                t.zlvl[p][l] = splitmix(&mut s);
            }
        }
        for k in 0..16 {
            for c in 0..CELLS {
                t.rzob[k][c] = splitmix(&mut s);
            }
            for l in 0..8 {
                t.rlvl[k][l] = splitmix(&mut s);
            }
        }
        t.zside = splitmix(&mut s);
        for i in 0..29 {
            t.zlast[i] = splitmix(&mut s);
        }
        t
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Bug {
    Queen = 0,
    Spider = 1,
    Beetle = 2,
    Grasshopper = 3,
    Ant = 4,
    Mosquito = 5,
    Ladybug = 6,
    Pillbug = 7,
}

// Piece index within a color: Q, S1, S2, B1, B2, G1, G2, G3, A1, A2, A3, M, L, P
pub const KIND_BUG: [Bug; 14] = [
    Bug::Queen,
    Bug::Spider,
    Bug::Spider,
    Bug::Beetle,
    Bug::Beetle,
    Bug::Grasshopper,
    Bug::Grasshopper,
    Bug::Grasshopper,
    Bug::Ant,
    Bug::Ant,
    Bug::Ant,
    Bug::Mosquito,
    Bug::Ladybug,
    Bug::Pillbug,
];
pub const KIND_NUM: [u8; 14] = [0, 1, 2, 1, 2, 1, 2, 3, 1, 2, 3, 0, 0, 0];
pub const BUG_FIRST: [usize; 8] = [0, 1, 3, 5, 8, 11, 12, 13];
pub const BUG_COUNT: [usize; 8] = [1, 2, 2, 3, 3, 1, 1, 1];
pub const BUG_CHAR: [char; 8] = ['Q', 'S', 'B', 'G', 'A', 'M', 'L', 'P'];

#[inline(always)]
pub fn bug_of(p: u8) -> Bug {
    KIND_BUG[(p % 14) as usize]
}
#[inline(always)]
pub fn color_of(p: u8) -> u8 {
    p / 14
}

pub fn piece_name(p: u8) -> String {
    let c = if color_of(p) == 0 { 'w' } else { 'b' };
    let k = (p % 14) as usize;
    let b = KIND_BUG[k] as usize;
    if KIND_NUM[k] == 0 {
        format!("{}{}", c, BUG_CHAR[b])
    } else {
        format!("{}{}{}", c, BUG_CHAR[b], KIND_NUM[k])
    }
}

pub fn parse_piece(s: &str) -> Option<u8> {
    let b = s.as_bytes();
    if b.len() < 2 || b.len() > 3 {
        return None;
    }
    let color = match b[0] {
        b'w' => 0u8,
        b'b' => 1u8,
        _ => return None,
    };
    let bug = BUG_CHAR.iter().position(|&c| c as u8 == b[1])?;
    let num = if b.len() == 3 {
        (b[2] as char).to_digit(10)? as usize
    } else {
        0
    };
    let first = BUG_FIRST[bug];
    let count = BUG_COUNT[bug];
    let k = if count == 1 {
        if num != 0 && num != 1 {
            return None;
        }
        first
    } else {
        if num < 1 || num > count {
            return None;
        }
        first + num - 1
    };
    Some(color * 14 + k as u8)
}

/// A move: piece, from (NONE = from hand), to. Pass is a special value.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Hash)]
pub struct Move(pub u32);

impl Move {
    pub const PASS: Move = Move(0xFFFF_FFFF);
    pub const NULL: Move = Move(0);
    #[inline(always)]
    pub fn new(piece: u8, from: u16, to: u16) -> Move {
        // piece+1 so that Move(0) is never a real move
        Move(((piece as u32) + 1) | ((from as u32) << 5) | ((to as u32) << 21))
    }
    #[inline(always)]
    pub fn piece(self) -> u8 {
        ((self.0 & 31) - 1) as u8
    }
    #[inline(always)]
    pub fn from(self) -> u16 {
        ((self.0 >> 5) & 0xFFFF) as u16
    }
    #[inline(always)]
    pub fn to(self) -> u16 {
        (self.0 >> 21) as u16
    }
    #[inline(always)]
    pub fn is_place(self) -> bool {
        self.from() == NONE
    }
    #[inline(always)]
    pub fn is_pass(self) -> bool {
        self == Move::PASS
    }
}

#[derive(Clone, Copy, Default)]
pub struct GameType {
    pub m: bool,
    pub l: bool,
    pub p: bool,
}

impl GameType {
    pub fn includes_kind(&self, k: usize) -> bool {
        match KIND_BUG[k] {
            Bug::Mosquito => self.m,
            Bug::Ladybug => self.l,
            Bug::Pillbug => self.p,
            _ => true,
        }
    }
    pub fn to_string(&self) -> String {
        if !self.m && !self.l && !self.p {
            return "Base".into();
        }
        let mut s = String::from("Base+");
        if self.m {
            s.push('M')
        }
        if self.l {
            s.push('L')
        }
        if self.p {
            s.push('P')
        }
        s
    }
    pub fn parse(s: &str) -> Option<GameType> {
        if s == "Base" {
            return Some(GameType::default());
        }
        let rest = s.strip_prefix("Base+")?;
        if rest.is_empty() {
            return None;
        }
        let mut g = GameType::default();
        let mut last = 0;
        for ch in rest.chars() {
            let ord = match ch {
                'M' => 1,
                'L' => 2,
                'P' => 3,
                _ => return None,
            };
            if ord <= last {
                return None;
            }
            last = ord;
            match ch {
                'M' => g.m = true,
                'L' => g.l = true,
                _ => g.p = true,
            }
        }
        Some(g)
    }
}

#[derive(Clone, Copy)]
pub struct Undo {
    pub mv: Move,
    pub last: u8,
}

#[derive(Clone)]
pub struct Board {
    pub gt: GameType,
    pub pos: [u16; 28],
    pub under: [u8; 28],
    pub lvl: [u8; 28],
    pub hgt: [u8; CELLS],
    pub top: [u8; CELLS],
    pub ply: u32,
    pub last: u8, // piece physically moved on previous turn (immobilized), NOP if none
    pub hash: u64,
    pub rhash: u64,
    pub placed: [u8; 2], // number of pieces placed per color
    pub occupied: u32,   // number of occupied cells
    pub history: Vec<u64>, // repetition hashes of previous positions (excluding current)
    pub pin_cache: std::cell::Cell<(u64, u32)>,
}

impl Board {
    pub fn new(gt: GameType) -> Board {
        tables();
        Board {
            gt,
            pos: [NONE; 28],
            under: [NOP; 28],
            lvl: [0; 28],
            hgt: [0; CELLS],
            top: [NOP; CELLS],
            ply: 0,
            last: NOP,
            hash: 0,
            rhash: 0,
            placed: [0; 2],
            occupied: 0,
            history: Vec::with_capacity(512),
            pin_cache: std::cell::Cell::new((u64::MAX, 0)),
        }
    }

    /// A position without a move history (the viewer's "Set up position"): `side` to move at pair-turn
    /// `turn` (as in a TurnString), pieces as (piece, q, r, stack level), and optionally the piece that
    /// was moved on the previous turn (it may not be moved again by a Pillbug this turn).
    pub fn from_setup(gt: GameType, side: u8, turn: u32, pieces: &[(u8, i32, i32, usize)], last: Option<u8>) -> Result<Board, String> {
        if !(1..=200).contains(&turn) {
            return Err("Choose a turn number from 1 to 200.".into());
        }
        let mut b = Board::new(gt);
        b.ply = (turn - 1) * 2 + side as u32;
        if pieces.len() > 28 {
            return Err("Too many pieces.".into());
        }
        // The grid wraps at 32, so work relative to one of the pieces: a connected hive of 28 always fits.
        let (q0, r0) = pieces.first().map(|x| (x.1, x.2)).unwrap_or((0, 0));
        let mut ordered = pieces.to_vec();
        ordered.sort_by_key(|x| (x.1, x.2, x.3));
        for &(p, q, r, level) in &ordered {
            if (q - q0).abs() > 14 || (r - r0).abs() > 14 {
                return Err("Connect every piece into one hive before analyzing.".into());
            }
            let kind_ok = match bug_of(p) {
                Bug::Mosquito => gt.m,
                Bug::Ladybug => gt.l,
                Bug::Pillbug => gt.p,
                _ => true,
            };
            if !kind_ok || b.pos[p as usize] != NONE {
                return Err("A piece is duplicated or unavailable in this game type.".into());
            }
            let c = cell(q - q0, r - r0);
            if level != b.hgt[c as usize] as usize || level >= 7 {
                return Err("Stack levels must be consecutive, with no gaps.".into());
            }
            if level > 0 && !matches!(bug_of(p), Bug::Beetle | Bug::Mosquito) {
                return Err("Only Beetles and Mosquitoes can be above another piece.".into());
            }
            b.put(p, c);
            b.placed[color_of(p) as usize] += 1;
        }
        for color in 0..2u8 {
            if b.placed[color as usize] as u32 > b.turns_taken(color) {
                return Err("Increase the turn number to allow all the placed pieces.".into());
            }
            if b.turns_taken(color) >= 4 && !b.queen_placed(color) {
                return Err(format!("Place the {} Queen, or choose an earlier turn number.", if color == 0 { "White" } else { "Black" }));
            }
        }
        // One hive: flood fill over occupied cells.
        if let Some(&(p, ..)) = ordered.first() {
            let t = tables();
            let start = b.pos[p as usize];
            let mut seen = vec![start];
            let mut i = 0;
            while i < seen.len() {
                let c = seen[i];
                i += 1;
                for d in 0..6 {
                    let n = t.nb[c as usize][d];
                    if b.hgt[n as usize] > 0 && !seen.contains(&n) {
                        seen.push(n);
                    }
                }
            }
            if seen.len() as u32 != b.occupied {
                return Err("Connect every piece into one hive before analyzing.".into());
            }
        }
        if let Some(p) = last {
            let c = b.pos[p as usize];
            if c == NONE {
                return Err("The last moved piece must be on the board.".into());
            }
            if b.top[c as usize] != p {
                return Err("The last moved piece must be on top of its stack.".into());
            }
            if b.ply == 0 {
                return Err("Choose a different previous hex for the last moved piece.".into());
            }
            b.last = p;
        }
        Ok(b)
    }

    #[inline(always)]
    pub fn side(&self) -> u8 {
        (self.ply & 1) as u8
    }
    #[inline(always)]
    pub fn turns_taken(&self, color: u8) -> u32 {
        (self.ply + 1 - color as u32) / 2
    }
    #[inline(always)]
    pub fn queen_placed(&self, color: u8) -> bool {
        self.pos[(color * 14) as usize] != NONE
    }
    #[inline(always)]
    pub fn key(&self) -> u64 {
        let t = tables();
        let mut k = self.hash ^ t.zlast[if self.last == NOP { 28 } else { self.last as usize }];
        if self.side() == 1 {
            k ^= t.zside;
        }
        k
    }
    /// Opening-book key: board + side to move (ignores the last-moved restriction).
    pub fn book_key(&self) -> u64 {
        let t = tables();
        if self.side() == 1 { self.hash ^ t.zside } else { self.hash }
    }

    #[inline(always)]
    pub fn rep_key(&self) -> u64 {
        let t = tables();
        let mut k = self.rhash;
        if self.gt.p {
            k ^= t.zlast[if self.last == NOP { 28 } else { self.last as usize }];
        }
        if self.side() == 1 {
            k ^= t.zside;
        }
        k
    }

    #[inline(always)]
    fn rkind(p: u8) -> usize {
        (color_of(p) as usize) * 8 + bug_of(p) as usize
    }

    #[inline(always)]
    fn put(&mut self, p: u8, c: u16) {
        let t = tables();
        let ci = c as usize;
        let h = self.hgt[ci];
        self.under[p as usize] = self.top[ci];
        self.lvl[p as usize] = h;
        self.top[ci] = p;
        if h == 0 {
            self.occupied += 1;
        }
        self.hgt[ci] = h + 1;
        self.pos[p as usize] = c;
        let l = (h as usize).min(7);
        self.hash ^= t.zob[p as usize][ci] ^ t.zlvl[p as usize][l];
        let rk = Self::rkind(p);
        self.rhash ^= t.rzob[rk][ci] ^ t.rlvl[rk][l];
    }

    #[inline(always)]
    fn take(&mut self, p: u8) -> u16 {
        let t = tables();
        let c = self.pos[p as usize];
        let ci = c as usize;
        debug_assert_eq!(self.top[ci], p);
        let h = self.hgt[ci] - 1;
        self.hgt[ci] = h;
        self.top[ci] = self.under[p as usize];
        if h == 0 {
            self.occupied -= 1;
        }
        self.under[p as usize] = NOP;
        self.pos[p as usize] = NONE;
        let l = (h as usize).min(7);
        self.hash ^= t.zob[p as usize][ci] ^ t.zlvl[p as usize][l];
        let rk = Self::rkind(p);
        self.rhash ^= t.rzob[rk][ci] ^ t.rlvl[rk][l];
        c
    }

    pub fn make(&mut self, mv: Move) -> Undo {
        let u = Undo { mv, last: self.last };
        self.history.push(self.rep_key());
        if mv.is_pass() {
            self.last = NOP;
        } else {
            let p = mv.piece();
            if mv.is_place() {
                self.put(p, mv.to());
                self.placed[color_of(p) as usize] += 1;
                self.last = NOP;
            } else {
                self.take(p);
                self.put(p, mv.to());
                self.last = p;
            }
        }
        self.ply += 1;
        u
    }

    pub fn unmake(&mut self, u: Undo) {
        self.ply -= 1;
        self.history.pop();
        let mv = u.mv;
        if !mv.is_pass() {
            let p = mv.piece();
            self.take(p);
            if mv.is_place() {
                self.placed[color_of(p) as usize] -= 1;
            } else {
                self.put(p, mv.from());
            }
        }
        self.last = u.last;
    }

    /// Number of occupied neighbors of the queen of `color` (0 if not placed).
    #[inline(always)]
    pub fn queen_neighbors(&self, color: u8) -> u32 {
        let q = self.pos[(color * 14) as usize];
        if q == NONE {
            return 0;
        }
        let t = tables();
        let mut n = 0;
        for d in 0..6 {
            if self.hgt[t.nb[q as usize][d] as usize] > 0 {
                n += 1;
            }
        }
        n
    }

    /// 0 = in progress, 1 = white wins, 2 = black wins, 3 = draw (both surrounded)
    pub fn result(&self) -> u8 {
        let w = self.queen_neighbors(0) == 6;
        let b = self.queen_neighbors(1) == 6;
        match (w, b) {
            (false, false) => 0,
            (true, true) => 3,
            (true, false) => 2,
            (false, true) => 1,
        }
    }

    /// Count of earlier occurrences of the current repetition key.
    pub fn repetitions(&self) -> usize {
        let k = self.rep_key();
        self.history.iter().filter(|&&h| h == k).count()
    }

    // ------------------------------------------------------------------
    // Articulation points (cells whose single piece cannot be lifted)
    // ------------------------------------------------------------------

    /// Returns a bitset over piece ids: pieces that are pinned by the One Hive rule
    /// (ground pieces whose cell is a cut vertex of the occupied-cell graph).
    pub fn pinned_mask(&self) -> u32 {
        let h = self.hash;
        let (ch, cm) = self.pin_cache.get();
        if ch == h {
            return cm;
        }
        let m = self.pinned_mask_raw();
        self.pin_cache.set((h, m));
        m
    }

    fn pinned_mask_raw(&self) -> u32 {
        let t = tables();
        let mut cells = [0u16; 28];
        let mut n = 0usize;
        for p in 0..28u8 {
            let c = self.pos[p as usize];
            if c != NONE && self.lvl[p as usize] == 0 {
                cells[n] = c;
                n += 1;
            }
        }
        if n <= 2 {
            return 0;
        }
        // map cell -> local index using a small linear search over neighbors via top piece ids
        // local index by bottom piece id: we use idx_of[cell] via a tiny scan map
        let mut local = [0u8; CELLS];
        // mark with index+1
        for i in 0..n {
            local[cells[i] as usize] = (i + 1) as u8;
        }
        let mut adj = [0u32; 28];
        for i in 0..n {
            let c = cells[i] as usize;
            for d in 0..6 {
                let j = local[t.nb[c][d] as usize];
                if j != 0 {
                    adj[i] |= 1 << (j - 1);
                }
            }
        }
        // iterative Tarjan from node 0
        let mut disc = [0u8; 28];
        let mut low = [0u8; 28];
        let mut parent = [255u8; 28];
        let mut iter_mask = [0u32; 28];
        let mut art = 0u32;
        let mut time = 1u8;
        let mut stack = [0u8; 28];
        let mut sp = 0usize;
        let mut root_children = 0;
        disc[0] = time;
        low[0] = time;
        time += 1;
        iter_mask[0] = adj[0];
        stack[sp] = 0;
        sp += 1;
        while sp > 0 {
            let u = stack[sp - 1] as usize;
            if iter_mask[u] != 0 {
                let v = iter_mask[u].trailing_zeros() as usize;
                iter_mask[u] &= iter_mask[u] - 1;
                if disc[v] == 0 {
                    parent[v] = u as u8;
                    disc[v] = time;
                    low[v] = time;
                    time += 1;
                    iter_mask[v] = adj[v];
                    stack[sp] = v as u8;
                    sp += 1;
                    if u == 0 {
                        root_children += 1;
                    }
                } else if parent[u] as usize != v {
                    if disc[v] < low[u] {
                        low[u] = disc[v];
                    }
                }
            } else {
                sp -= 1;
                if sp > 0 {
                    let p = stack[sp - 1] as usize;
                    if low[u] < low[p] {
                        low[p] = low[u];
                    }
                    if p != 0 && low[u] >= disc[p] {
                        art |= 1 << p;
                    }
                }
            }
        }
        if root_children > 1 {
            art |= 1;
        }
        let mut mask = 0u32;
        let mut a = art;
        while a != 0 {
            let i = a.trailing_zeros() as usize;
            a &= a - 1;
            let c = cells[i] as usize;
            if self.hgt[c] == 1 {
                mask |= 1 << self.top[c];
            }
        }
        mask
    }

    // ------------------------------------------------------------------
    // Move generation
    // ------------------------------------------------------------------

    #[inline(always)]
    fn h(&self, c: u16) -> u8 {
        self.hgt[c as usize]
    }

    /// General step test from a to b=nb(a,d), with the mover already lifted.
    /// `ha` is the height of the source stack (mover excluded).
    #[inline(always)]
    fn step_ok(&self, a: u16, d: usize, ha: u8) -> bool {
        let t = tables();
        let nb = &t.nb[a as usize];
        let b = nb[d];
        let hb = self.h(b);
        let hl = self.h(nb[(d + 5) % 6]);
        let hr = self.h(nb[(d + 1) % 6]);
        let th = ha.max(hb);
        if th == 0 && hl == 0 && hr == 0 {
            return false;
        }
        !(hl > th && hr > th)
    }

    /// Ground slide test (both ends empty at ground, mover lifted).
    #[inline(always)]
    fn slide_ok(&self, a: u16, d: usize) -> bool {
        self.slide_ok_t(tables(), a, d)
    }

    #[inline(always)]
    fn slide_ok_t(&self, t: &Tables, a: u16, d: usize) -> bool {
        let nb = &t.nb[a as usize];
        if self.h(nb[d]) != 0 {
            return false;
        }
        (self.h(nb[(d + 5) % 6]) != 0) ^ (self.h(nb[(d + 1) % 6]) != 0)
    }

    pub fn legal_moves(&mut self, out: &mut Vec<Move>) {
        out.clear();
        if self.result() != 0 {
            return;
        }
        self.gen_placements(out);
        let color = self.side();
        if self.queen_placed(color) {
            let pinned = self.pinned_mask();
            let base = (color * 14) as usize;
            for k in 0..14 {
                if !self.gt.includes_kind(k) {
                    continue;
                }
                let p = (base + k) as u8;
                let c = self.pos[p as usize];
                if c == NONE || self.top[c as usize] != p || p == self.last {
                    continue;
                }
                if self.lvl[p as usize] == 0 && (pinned >> p) & 1 != 0 {
                    // pinned: no movement, but pillbug ability may still apply below
                } else {
                    self.gen_piece_moves(p, out);
                }
            }
            if self.gt.p {
                for k in [11usize, 13usize] {
                    let p = (base + k) as u8;
                    if !self.gt.includes_kind(k) {
                        continue;
                    }
                    self.gen_pillbug_ability(p, pinned, out);
                }
            }
        }
        if out.is_empty() {
            out.push(Move::PASS);
        }
    }

    fn gen_placements(&self, out: &mut Vec<Move>) {
        let color = self.side();
        let turns = self.turns_taken(color);
        let first = turns == 0;
        let force_queen = turns >= 3 && !self.queen_placed(color);
        let mut pieces = [0u8; 8];
        let mut n = 0;
        let base = color * 14;
        for b in 0..8usize {
            let first_k = BUG_FIRST[b];
            if !self.gt.includes_kind(first_k) {
                continue;
            }
            if first && b == 0 {
                continue;
            }
            if force_queen && b != 0 {
                continue;
            }
            for i in 0..BUG_COUNT[b] {
                let p = base + (first_k + i) as u8;
                if self.pos[p as usize] == NONE {
                    pieces[n] = p;
                    n += 1;
                    break;
                }
            }
        }
        if n == 0 {
            return;
        }
        let t = tables();
        if self.ply == 0 {
            let c = cell(0, 0);
            for &p in &pieces[..n] {
                out.push(Move::new(p, NONE, c));
            }
            return;
        }
        if self.ply == 1 {
            let wc = (0..28).map(|i| self.pos[i]).find(|&c| c != NONE).unwrap();
            for d in 0..6 {
                let c = t.nb[wc as usize][d];
                for &p in &pieces[..n] {
                    out.push(Move::new(p, NONE, c));
                }
            }
            return;
        }
        let mut seen = [false; CELLS];
        for q in 0..28u8 {
            let c = self.pos[q as usize];
            if c == NONE || self.top[c as usize] != q || color_of(q) != color {
                continue;
            }
            for d in 0..6 {
                let e = t.nb[c as usize][d];
                if self.hgt[e as usize] != 0 || seen[e as usize] {
                    continue;
                }
                seen[e as usize] = true;
                let mut ok = true;
                for d2 in 0..6 {
                    let f = t.nb[e as usize][d2] as usize;
                    if self.hgt[f] != 0 && color_of(self.top[f]) != color {
                        ok = false;
                        break;
                    }
                }
                if ok {
                    for &p in &pieces[..n] {
                        out.push(Move::new(p, NONE, e));
                    }
                }
            }
        }
    }

    pub fn gen_piece_moves(&mut self, p: u8, out: &mut Vec<Move>) {
        let src = self.pos[p as usize];
        let start = out.len();
        // Lift the piece temporarily (without touching hashes).
        let si = src as usize;
        let saved_top = self.top[si];
        let saved_h = self.hgt[si];
        self.hgt[si] = saved_h - 1;
        self.top[si] = self.under[p as usize];
        let ha = saved_h - 1;
        let bug = bug_of(p);
        if ha > 0 {
            // On top of the hive: beetle or mosquito-as-beetle
            self.beetle_dests(p, src, ha, out);
        } else {
            match bug {
                Bug::Queen | Bug::Pillbug => self.slide1(p, src, out),
                Bug::Beetle => self.beetle_dests(p, src, 0, out),
                Bug::Grasshopper => self.hopper(p, src, out),
                Bug::Spider => self.spider(p, src, out),
                Bug::Ant => self.ant(p, src, out),
                Bug::Ladybug => self.ladybug(p, src, out),
                Bug::Mosquito => {
                    let t = tables();
                    let mut mask = 0u8;
                    for d in 0..6 {
                        let n = t.nb[si][d] as usize;
                        if self.hgt[n] != 0 {
                            let b = bug_of(self.top[n]);
                            mask |= 1 << (b as u8);
                        }
                    }
                    if mask & (1 << Bug::Ant as u8) != 0 {
                        self.ant(p, src, out);
                    } else if mask & ((1 << Bug::Queen as u8) | (1 << Bug::Pillbug as u8)) != 0 {
                        self.slide1(p, src, out);
                    }
                    if mask & (1 << Bug::Beetle as u8) != 0 {
                        self.beetle_dests(p, src, 0, out);
                    }
                    if mask & (1 << Bug::Grasshopper as u8) != 0 {
                        self.hopper(p, src, out);
                    }
                    if mask & (1 << Bug::Spider as u8) != 0 && mask & (1 << Bug::Ant as u8) == 0 {
                        self.spider(p, src, out);
                    }
                    if mask & (1 << Bug::Ladybug as u8) != 0 {
                        self.ladybug(p, src, out);
                    }
                }
            }
        }
        self.hgt[si] = saved_h;
        self.top[si] = saved_top;
        // dedupe destinations for this piece (only multi-path generators can repeat)
        let end = out.len();
        let needs = matches!(bug, Bug::Spider | Bug::Ladybug | Bug::Mosquito);
        if needs && end - start > 1 {
            let slice = &mut out[start..end];
            slice.sort_unstable_by_key(|m| m.0);
            let mut w = start + 1;
            for r in start + 1..end {
                if out[r] != out[w - 1] {
                    out[w] = out[r];
                    w += 1;
                }
            }
            out.truncate(w);
        }
    }

    fn slide1(&self, p: u8, src: u16, out: &mut Vec<Move>) {
        let t = tables();
        for d in 0..6 {
            if self.slide_ok(src, d) {
                out.push(Move::new(p, src, t.nb[src as usize][d]));
            }
        }
    }

    fn beetle_dests(&self, p: u8, src: u16, ha: u8, out: &mut Vec<Move>) {
        let t = tables();
        for d in 0..6 {
            if self.step_ok(src, d, ha) {
                out.push(Move::new(p, src, t.nb[src as usize][d]));
            }
        }
    }

    fn hopper(&self, p: u8, src: u16, out: &mut Vec<Move>) {
        let t = tables();
        for d in 0..6 {
            let mut c = t.nb[src as usize][d];
            if self.h(c) == 0 {
                continue;
            }
            while self.h(c) != 0 {
                c = t.nb[c as usize][d];
            }
            out.push(Move::new(p, src, c));
        }
    }

    fn spider(&self, p: u8, src: u16, out: &mut Vec<Move>) {
        let t = tables();
        for d1 in 0..6 {
            if !self.slide_ok_t(t, src, d1) {
                continue;
            }
            let c1 = t.nb[src as usize][d1];
            for d2 in 0..6 {
                if !self.slide_ok_t(t, c1, d2) {
                    continue;
                }
                let c2 = t.nb[c1 as usize][d2];
                if c2 == src {
                    continue;
                }
                for d3 in 0..6 {
                    if !self.slide_ok_t(t, c2, d3) {
                        continue;
                    }
                    let c3 = t.nb[c2 as usize][d3];
                    if c3 == c1 || c3 == src {
                        continue;
                    }
                    out.push(Move::new(p, src, c3));
                }
            }
        }
    }

    fn ant(&self, p: u8, src: u16, out: &mut Vec<Move>) {
        let t = tables();
        let mut visited = [0u64; CELLS / 64];
        let mut queue = [0u16; 128];
        let (mut head, mut tail) = (0usize, 0usize);
        visited[src as usize >> 6] |= 1 << (src & 63);
        queue[tail] = src;
        tail += 1;
        while head < tail {
            let c = queue[head];
            head += 1;
            for d in 0..6 {
                if !self.slide_ok_t(t, c, d) {
                    continue;
                }
                let n = t.nb[c as usize][d];
                let bit = 1u64 << (n & 63);
                if visited[n as usize >> 6] & bit != 0 {
                    continue;
                }
                visited[n as usize >> 6] |= bit;
                if tail < queue.len() {
                    queue[tail] = n;
                    tail += 1;
                }
                out.push(Move::new(p, src, n));
            }
        }
    }

    fn ladybug(&self, p: u8, src: u16, out: &mut Vec<Move>) {
        let t = tables();
        for d1 in 0..6 {
            let c1 = t.nb[src as usize][d1];
            if self.h(c1) == 0 || !self.step_ok(src, d1, 0) {
                continue;
            }
            let h1 = self.h(c1);
            for d2 in 0..6 {
                let c2 = t.nb[c1 as usize][d2];
                if self.h(c2) == 0 || !self.step_ok(c1, d2, h1) {
                    continue;
                }
                let h2 = self.h(c2);
                for d3 in 0..6 {
                    let c3 = t.nb[c2 as usize][d3];
                    if self.h(c3) != 0 || c3 == src || !self.step_ok(c2, d3, h2) {
                        continue;
                    }
                    out.push(Move::new(p, src, c3));
                }
            }
        }
    }

    fn gen_pillbug_ability(&self, actor: u8, pinned: u32, out: &mut Vec<Move>) {
        let a = self.pos[actor as usize];
        if a == NONE || actor == self.last || self.hgt[a as usize] != 1 {
            return;
        }
        let t = tables();
        let nb = &t.nb[a as usize];
        if bug_of(actor) == Bug::Mosquito {
            let mut ok = false;
            for d in 0..6 {
                let n = nb[d] as usize;
                if self.hgt[n] != 0 && bug_of(self.top[n]) == Bug::Pillbug {
                    ok = true;
                    break;
                }
            }
            if !ok {
                return;
            }
        }
        let hs: [u8; 6] = std::array::from_fn(|i| self.hgt[nb[i] as usize]);
        let gate: [bool; 6] = std::array::from_fn(|i| !(hs[(i + 5) % 6] > 1 && hs[(i + 1) % 6] > 1));
        for i in 0..6 {
            if hs[i] != 1 || !gate[i] {
                continue;
            }
            let src = nb[i];
            let q = self.top[src as usize];
            if q == self.last || (pinned >> q) & 1 != 0 {
                continue;
            }
            for j in 0..6 {
                if j == i || hs[j] != 0 || !gate[j] {
                    continue;
                }
                let m = Move::new(q, src, nb[j]);
                // avoid duplicate if another actor already produced it
                if !out.contains(&m) {
                    out.push(m);
                }
            }
        }
    }

    /// Pillbug relocation defence for `color`: (enemy queen-attackers that can be
    /// thrown off the queen's ring [max 2], whether the queen itself can be relocated
    /// to a less-surrounded cell).
    pub fn relocation_defense(&self, color: u8) -> (i32, bool) {
        if !self.gt.p {
            return (0, false);
        }
        let t = tables();
        let queen = color * 14;
        let q = self.pos[queen as usize];
        if q == NONE {
            return (0, false);
        }
        let qn = &t.nb[q as usize];
        let occ = qn.iter().filter(|&&c| self.hgt[c as usize] != 0).count();
        if occ == 6 {
            return (0, false);
        }
        let immob = if self.side() == color { self.last } else { NOP };
        let mut pinned: Option<u32> = None;
        let mut removed = 0u32;
        let mut escape = false;
        for k in [13u8, 11u8] {
            if !self.gt.includes_kind(k as usize) {
                continue;
            }
            let actor = color * 14 + k;
            let h = self.pos[actor as usize];
            if h == NONE || actor == immob || self.top[h as usize] != actor || self.hgt[h as usize] != 1 {
                continue;
            }
            if crate::eval::dist(h, q) > 2 {
                continue;
            }
            let ring = &t.nb[h as usize];
            if k == 11 && !ring.iter().any(|&c| self.hgt[c as usize] != 0 && bug_of(self.top[c as usize]) == Bug::Pillbug) {
                continue;
            }
            let hs: [u8; 6] = std::array::from_fn(|i| self.hgt[ring[i] as usize]);
            let gate: [bool; 6] = std::array::from_fn(|i| !(hs[(i + 5) % 6] > 1 && hs[(i + 1) % 6] > 1));
            for i in 0..6 {
                if hs[i] != 1 || !gate[i] {
                    continue;
                }
                let from = ring[i];
                let piece = self.top[from as usize];
                if piece == immob {
                    continue;
                }
                let removes = color_of(piece) != color && qn.contains(&from) && removed & (1 << piece) == 0;
                let rescues = piece == queen && !escape;
                if !removes && !rescues {
                    continue;
                }
                let mut useful = false;
                for j in 0..6 {
                    if hs[j] != 0 || !gate[j] {
                        continue;
                    }
                    let to = ring[j];
                    if removes && !qn.contains(&to) {
                        useful = true;
                        break;
                    }
                    if rescues {
                        let n = t.nb[to as usize].iter().filter(|&&c| c != from && self.hgt[c as usize] != 0).count();
                        if n < occ {
                            useful = true;
                            break;
                        }
                    }
                }
                if !useful {
                    continue;
                }
                let pm = *pinned.get_or_insert_with(|| self.pinned_mask());
                if (pm >> piece) & 1 != 0 {
                    continue;
                }
                if removes {
                    removed |= 1 << piece;
                }
                if rescues {
                    escape = true;
                }
            }
        }
        ((removed.count_ones() as i32).min(2), escape)
    }

    /// Can `attacker` make a move that increases the number of occupied cells
    /// around the defender's queen (one-step ring-fill access)?
    /// Distinct empty cells around the defender's queen that `attacker` can fill with one
    /// movement, and the number of distinct attacker pieces able to do so.
    pub fn ring_fill(&mut self, attacker: u8, buf: &mut Vec<Move>) -> (u32, u32) {
        let t = tables();
        let dq = self.pos[((1 - attacker) * 14) as usize];
        if dq == NONE || !self.queen_placed(attacker) {
            return (0, 0);
        }
        let ring = t.nb[dq as usize];
        let mut empty = 0u8;
        for d in 0..6 {
            if self.hgt[ring[d] as usize] == 0 {
                empty |= 1 << d;
            }
        }
        if empty == 0 {
            return (0, 0);
        }
        let pinned = self.pinned_mask();
        let base = attacker * 14;
        let mut cells = 0u8;
        let mut pieces = 0u32;
        for k in 0..14u8 {
            if !self.gt.includes_kind(k as usize) {
                continue;
            }
            let p = base + k;
            let c = self.pos[p as usize];
            if c == NONE || self.top[c as usize] != p {
                continue;
            }
            if self.lvl[p as usize] == 0 && (pinned >> p) & 1 != 0 {
                continue;
            }
            if ring.contains(&c) && self.hgt[c as usize] == 1 {
                continue;
            }
            buf.clear();
            self.gen_piece_moves(p, buf);
            let mut any = false;
            for m in buf.iter() {
                let to = m.to();
                if let Some(d) = ring.iter().position(|&r| r == to) {
                    if empty & (1 << d) != 0 {
                        cells |= 1 << d;
                        any = true;
                    }
                }
            }
            if any {
                pieces += 1;
            }
        }
        (cells.count_ones(), pieces)
    }

    pub fn ring_access(&mut self, attacker: u8, buf: &mut Vec<Move>) -> bool {
        let t = tables();
        let dq = self.pos[((1 - attacker) * 14) as usize];
        if dq == NONE || !self.queen_placed(attacker) {
            return false;
        }
        let ring = t.nb[dq as usize];
        let mut empty = 0u8;
        for d in 0..6 {
            if self.hgt[ring[d] as usize] == 0 {
                empty |= 1 << d;
            }
        }
        if empty == 0 {
            return false;
        }
        let pinned = self.pinned_mask();
        let base = attacker * 14;
        let mut ant_tried = false;
        for k in 0..14u8 {
            if !self.gt.includes_kind(k as usize) {
                continue;
            }
            let p = base + k;
            let c = self.pos[p as usize];
            if c == NONE || self.top[c as usize] != p {
                continue;
            }
            if self.lvl[p as usize] == 0 && (pinned >> p) & 1 != 0 {
                continue;
            }
            // a ground piece already on the ring leaves a hole when it moves
            if ring.contains(&c) && self.hgt[c as usize] == 1 {
                continue;
            }
            // all free ants on the ground reach (nearly) the same perimeter: flood only once
            let is_ant = KIND_BUG[k as usize] == Bug::Ant && self.lvl[p as usize] == 0;
            if is_ant {
                if ant_tried {
                    continue;
                }
                ant_tried = true;
            }
            buf.clear();
            self.gen_piece_moves(p, buf);
            for m in buf.iter() {
                let to = m.to();
                if let Some(d) = ring.iter().position(|&r| r == to) {
                    if empty & (1 << d) != 0 {
                        return true;
                    }
                }
            }
        }
        false
    }

    // ------------------------------------------------------------------
    // UHP notation
    // ------------------------------------------------------------------

    pub fn move_to_string(&self, mv: Move) -> String {
        if mv.is_pass() {
            return "pass".into();
        }
        let p = mv.piece();
        let name = piece_name(p);
        if mv.is_place() && self.occupied == 0 {
            return name;
        }
        let to = mv.to();
        let t = tables();
        let from = mv.from();
        // destination occupied (after lifting) -> climb onto
        let top_after = |c: u16| -> u8 {
            if c == from {
                self.under[p as usize]
            } else {
                self.top[c as usize]
            }
        };
        let dest_top = top_after(to);
        if dest_top != NOP && to != from {
            return format!("{} {}", name, piece_name(dest_top));
        }
        let mut best: Option<(u8, usize)> = None;
        for d in 0..6 {
            let n = t.nb[to as usize][d];
            let q = top_after(n);
            if q == NOP || q == p {
                continue;
            }
            if best.map_or(true, |(bq, _)| q < bq) {
                best = Some((q, d));
            }
        }
        let (q, d) = best.expect("destination has a neighbor");
        // d is direction from destination to reference; we need direction from reference to destination
        let rd = (d + 3) % 6;
        let r = piece_name(q);
        match rd {
            0 => format!("{} {}-", name, r),
            1 => format!("{} {}/", name, r),
            2 => format!("{} \\{}", name, r),
            3 => format!("{} -{}", name, r),
            4 => format!("{} /{}", name, r),
            _ => format!("{} {}\\", name, r),
        }
    }

    /// Parse a UHP MoveString into a legal move, or an error message.
    pub fn parse_move(&mut self, s: &str) -> Result<Move, String> {
        let s = s.trim();
        let mut legal = Vec::new();
        self.legal_moves(&mut legal);
        if s.eq_ignore_ascii_case("pass") {
            if legal.len() == 1 && legal[0].is_pass() {
                return Ok(Move::PASS);
            }
            return Err("Passing is only allowed when no other move is available.".into());
        }
        let mut parts = s.split_whitespace();
        let pstr = parts.next().ok_or("Empty MoveString.")?;
        let p = parse_piece(pstr).ok_or_else(|| format!("Unknown piece '{}'.", pstr))?;
        let target = parts.next();
        if parts.next().is_some() {
            return Err("Too many fields in MoveString.".into());
        }
        let t = tables();
        let to: u16 = match target {
            None => {
                if self.occupied != 0 {
                    return Err("A destination is required after the first move.".into());
                }
                cell(0, 0)
            }
            Some(tg) => {
                let (dirpre, rest) = match tg.chars().next() {
                    Some('-') => (Some(3usize), &tg[1..]),
                    Some('/') => (Some(4usize), &tg[1..]),
                    Some('\\') => (Some(2usize), &tg[1..]),
                    _ => (None, tg),
                };
                let (dirpost, name) = if dirpre.is_none() {
                    match rest.chars().last() {
                        Some('-') => (Some(0usize), &rest[..rest.len() - 1]),
                        Some('/') => (Some(1usize), &rest[..rest.len() - 1]),
                        Some('\\') => (Some(5usize), &rest[..rest.len() - 1]),
                        _ => (None, rest),
                    }
                } else {
                    (None, rest)
                };
                let r = parse_piece(name).ok_or_else(|| format!("Unknown reference piece '{}'.", name))?;
                let rc = self.pos[r as usize];
                if rc == NONE {
                    return Err(format!("Reference piece {} is not on the board.", name));
                }
                match dirpre.or(dirpost) {
                    None => rc,
                    Some(d) => t.nb[rc as usize][d],
                }
            }
        };
        let from = self.pos[p as usize];
        for &m in &legal {
            if !m.is_pass() && m.piece() == p && m.to() == to && m.from() == from {
                return Ok(m);
            }
        }
        Err(format!("'{}' is not a legal move.", s))
    }

    pub fn perft(&mut self, depth: u32) -> u64 {
        if depth == 0 {
            return 1;
        }
        let mut moves = Vec::with_capacity(256);
        self.legal_moves(&mut moves);
        if depth == 1 {
            return moves.len() as u64;
        }
        let mut n = 0;
        for &m in &moves {
            let u = self.make(m);
            n += self.perft(depth - 1);
            self.unmake(u);
        }
        n
    }
}
