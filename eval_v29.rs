//! Handcrafted evaluation. Score is from the side to move's perspective, in
//! centi-units (roughly: 100 = a clear but small edge, 300 = large edge).

use crate::board::*;

pub const NF: usize = 58;
// Feature layout (white minus black):
//  0..5   own queen with 1..5 occupied neighbours (one-hot)
//  5      own queen can slide
//  6..13  movable piece of bug type 1..7 (6+bug-1)
//  13..21 piece in hand of bug type 0..7
//  21     our beetle/mosquito on top of the enemy queen
//  22     our piece covered by an enemy piece
//  23     our pillbug next to our queen (can throw attackers)
//  24     pinned enemy pieces next to our queen
//  25     free friendly pieces next to our queen x occupancy
//  26     pieces on board
//  27     proximity to enemy queen (sum of 6 - dist)
//  28     placement spots (capped at 4)
//  29..34 freedom 0..4 (one-hot; movable + placement spots)
//  34     queen still in hand after turn 2
//  35     enemy beetle (or mosquito on hive) next to our queen, free to climb
//  36     our queen cannot move while 2+ neighbours
//  37..42 own queen 1..5 neighbours but enemy has no ring access (replaces 0..5)
//  42     enemy pieces that are pinned (one-hive)
//  43     pillbug can throw enemy attackers off our queen (max 2)
//  44     pillbug can relocate our queen to safer cell
//  45     pillbug in hand and placeable next to our queen
//  46     min(0, our available bugs - vacancies around enemy queen), floor -4
//  47     free jumpers next to our queen
//  48     direct-drop spots around enemy queen (cap 2, needs pieces in hand)
//  49     our piece on top of enemy pillbug
//  50     own pinned pieces next to own queen
//  51     fillable cells around enemy queen
//  52     pieces able to fill (max 3)
//  53     free enemy beetle/mosquito-with-beetle-power at distance 2 from our queen
//  54     enemy drop spots within 2 of our queen (enemy has beetle/mosquito in hand), cap 3
//  55     free mosquito with ant power
//  56     our piece on top of an enemy beetle
//  57     tempo (side to move)
pub static DEFAULT_W: [i32; NF] = [
    -20, -55, -110, -220, -450, // danger
    14, // queen free
    6, 12, 8, 16, 12, 10, 8, // mobile S B G A M L P
    0, 0, 0, 0, 0, 0, 0, 0, // hand Q S B G A M L P
    100, -18, 22, -10, 3, // beetle on queen, covered, guard, attacker pinned, escape
    12, 3, 25, // on board, proximity, spots
    -220, -130, -70, -35, -12, // cramped
    -10, // no queen
    -60, // enemy beetle next to our queen
    -40, // our queen cannot move
    -20, -20, -20, -25, -60, // danger without enemy ring access (occ 1..5)
    12, // enemy non-queen pieces pinned
    18, 24, // pillbug relocation defence: attackers removable, queen escape
    40, // pillbug in hand and placeable next to our queen
    25, // attack strength shortfall (<=0): available bugs minus vacancies around enemy queen
    8, // free jumpers (B G L M) next to our own queen
    20, // direct-drop spots: empty kill spots of the enemy queen we can place into (Groundwork)
    30, // our beetle/mosquito covering the enemy pillbug ("qualifying for the win")
    -12, // our own pinned pieces next to our queen ("true pin" - spot filled for good)
    0, // empty cells around the enemy queen we can fill with one movement
    0, // our pieces able to fill a cell around the enemy queen (max 3)
    0, // free enemy beetle (or beetle-powered mosquito) two steps from our queen
    0, // enemy placement spots within two of our queen while it holds a beetle/mosquito
    0, // our free mosquito with ant power (touching an ant)
    0, // our piece sitting on top of an enemy beetle (beetle neutralised)
    6, // tempo
];

pub fn weights() -> &'static [i32; NF] {
    use std::sync::OnceLock;
    static W: OnceLock<[i32; NF]> = OnceLock::new();
    W.get_or_init(|| {
        let mut w = TUNED_W.unwrap_or(DEFAULT_W);
        if let Ok(path) = std::env::var("TR_WEIGHTS") {
            if let Ok(text) = std::fs::read_to_string(path) {
                let v: Vec<i32> = text.split(|c: char| c == ',' || c.is_whitespace()).filter_map(|x| x.parse().ok()).collect();
                if v.len() == NF {
                    w.copy_from_slice(&v);
                }
            }
        }
        w
    })
}

pub static TUNED_W: Option<[i32; NF]> = None;

#[inline(always)]
pub fn dist(a: u16, b: u16) -> i32 {
    let (aq, ar) = ((a & 31) as i32, (a >> 5) as i32);
    let (bq, br) = ((b & 31) as i32, (b >> 5) as i32);
    let dq = ((aq - bq + 16) & 31) - 16;
    let dr = ((ar - br + 16) & 31) - 16;
    (dq.abs() + dr.abs() + (dq + dr).abs()) / 2
}

thread_local! {
    static RING_BUF: std::cell::RefCell<Vec<Move>> = std::cell::RefCell::new(Vec::with_capacity(256));
}

/// Evaluate a non-terminal position from the perspective of the side to move.
pub fn evaluate(b: &mut Board) -> i32 {
    if simple_mode() {
        return evaluate_simple(b);
    }
    let key = b.key();
    let cacheable = b.ply >= 6 && caution() == 0;
    if cacheable {
        let hit = EVAL_CACHE.with(|c| {
            let c = c.borrow();
            let e = c[(key as usize) & (EC_SIZE - 1)];
            if e != 0 && (e ^ key) >> 32 == 0 { Some(e as u32 as i32) } else { None }
        });
        if let Some(v) = hit {
            return v;
        }
    }
    let v = evaluate_full(b);
    if cacheable {
        EVAL_CACHE.with(|c| {
            c.borrow_mut()[(key as usize) & (EC_SIZE - 1)] = (key & 0xFFFF_FFFF_0000_0000) | (v as u32 as u64);
        });
    }
    v
}

const EC_SIZE: usize = 1 << 18;
thread_local! {
    static EVAL_CACHE: std::cell::RefCell<Vec<u64>> = std::cell::RefCell::new(vec![0u64; EC_SIZE]);
}

fn evaluate_full(b: &mut Board) -> i32 {
    if let Some(n) = net() {
        let fb = features_both(b);
        let me = b.side() as usize;
        let mut ex = [0f32; EXTRA];
        if n.n_in > 2 * (NF - 1) {
            extra_inputs(b, &mut ex);
        }
        let mut v = n.eval(&fb[me], &fb[1 - me], &ex);
        let hw = hand_w();
        if hw.iter().any(|&w| w != 0) {
            v += hand_terms(b, &fb, me, &hw);
        }
        let fw = fort_w();
        if fw != 0 {
            v += fw * (fortress(b, me) - fortress(b, 1 - me));
        }
        let (gn, gf) = gb_w();
        if gn != 0 || gf != 0 {
            v += grounded_beetles(b, 1 - me, gn, gf) - grounded_beetles(b, me, gn, gf);
        }
        let qon = qon_w();
        if qon != 0 {
            // flat (not squared) penalty for an enemy piece sitting on a queen
            for (c, sign) in [(me, -1), (1 - me, 1)] {
                let q = b.pos[c * 14];
                if q != NONE && b.hgt[q as usize] > 1 && color_of(b.top[q as usize]) as usize != c {
                    // sign 1 = we sit on their queen: TR_QONA (default = TR_QON) so the bonus can be cut
                    v += if sign > 0 { qona_w() } else { -qon };
                }
            }
        }
        let (ew, eh) = early_w();
        if ew != 0 {
            // early lone raider: our ant/spider/grasshopper/ladybug touching the enemy queen while we
            // still have many pieces in hand (it gets pinned there; develop instead)
            let t = tables();
            for (c, sign) in [(me, 1), (1 - me, -1)] {
                let inhand = (0..14).filter(|&k| b.gt.includes_kind(k) && b.pos[c * 14 + k] == NONE).count() as i32;
                if inhand < eh { continue; }
                let eq = b.pos[(1 - c) * 14];
                if eq == NONE { continue; }
                for d in 0..6 {
                    let n = t.nb[eq as usize][d] as usize;
                    if b.hgt[n] == 1 && color_of(b.top[n]) as usize == c
                        && matches!(bug_of(b.top[n]), Bug::Ant | Bug::Spider | Bug::Grasshopper | Bug::Ladybug) {
                        v -= sign * ew;
                    }
                }
            }
        }
        let pbq = pbq_w();
        if pbq != 0 {
            // a queen without a working pillbug beside it (pillbug away, or buried by anyone)
            for (c, sign) in [(me, -1), (1 - me, 1)] {
                v += sign * pbq * pb_away(b, c);
            }
        }
        let qs = qd_scale();
        if qs != 0 {
            v += (qdanger(b, 1 - me) - qdanger(b, me)) * qs / 10;
        }
        let c = caution();
        if c != 0 {
            // Risk aversion: danger to the engine's own queen counts extra (no reckless attacks).
            let r = ROOT_COLOR.load(std::sync::atomic::Ordering::Relaxed) as usize;
            // TR_CAUTION_BLACK=1: only be cautious as Black (White has the initiative).
            let black_only = std::env::var("TR_CAUTION_BLACK").map(|v| v != "0").unwrap_or(false);
            let d = if black_only && r == 0 { 0 } else { queen_danger(b, r) };
            v += if me == r { -d * c / 100 } else { d * c / 100 };
        }
        return v;
    }
    let fb = features_both(b);
    let w = weights();
    let mut s = 0i32;
    for i in 0..NF - 1 {
        s += w[i] * (fb[0][i] - fb[1][i]);
    }
    let c = caution();
    if c != 0 {
        // Risk aversion for the engine's own colour: own-queen danger counts extra.
        let r = ROOT_COLOR.load(std::sync::atomic::Ordering::Relaxed) as usize;
        let mut d = 0i32;
        for &i in DANGER_IDX.iter() {
            d += w[i] * fb[r][i];
        }
        let extra = d * c / 100;
        s += if r == 0 { extra } else { -extra };
    }
    let s = if b.side() == 0 { s } else { -s };
    s + w[NF - 1]
}

pub static ROOT_COLOR: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
const DANGER_IDX: [usize; 12] = [0, 1, 2, 3, 4, 35, 36, 37, 38, 39, 40, 41];
fn caution() -> i32 {
    use std::sync::OnceLock;
    static C: OnceLock<i32> = OnceLock::new();
    *C.get_or_init(|| std::env::var("TR_CAUTION").ok().and_then(|v| v.parse().ok()).unwrap_or(0))
}

/// Feature vector from White's perspective (tempo slot left 0).
pub fn features(b: &mut Board) -> [i32; NF] {
    let f = features_both(b);
    let mut out = [0i32; NF];
    for i in 0..NF {
        out[i] = f[0][i] - f[1][i];
    }
    out
}

pub fn features_both(b: &mut Board) -> [[i32; NF]; 2] {
    let t = tables();
    let pinned = b.pinned_mask();
    let mut f = [[0i32; NF]; 2];
    let mut movable = [0i32; 2];
    let mut in_hand = [0i32; 2];
    for color in 0..2u8 {
        let ci = color as usize;
        let base = ci * 14;
        let qpos = b.pos[base];
        let opp = 1 - color;
        let own_queen = b.queen_placed(color);
        let eq = b.pos[(opp as usize) * 14];
        for k in 0..14 {
            if !b.gt.includes_kind(k) {
                continue;
            }
            let p = (base + k) as u8;
            let c = b.pos[p as usize];
            let bug = KIND_BUG[k] as usize;
            if c == NONE {
                f[ci][13 + bug] += 1;
                in_hand[ci] += 1;
                continue;
            }
            f[ci][26] += 1;
            if k != 0 && b.lvl[p as usize] == 0 && (pinned >> p) & 1 != 0 {
                f[opp as usize][42] += 1;
            }
            if k != 0 && eq != NONE {
                f[ci][27] += 6 - dist(c, eq).min(6);
            }
            if b.top[c as usize] != p {
                if color_of(b.top[c as usize]) != color {
                    f[ci][22] += 1;
                    if matches!(KIND_BUG[k], Bug::Beetle | Bug::Mosquito) {
                        f[opp as usize][56] += 1;
                    }
                    // a covered mosquito next to its own queen was (or could be) a pillbug-style guard
                    let own_q = b.pos[(ci * 14) as usize];
                    if KIND_BUG[k] == Bug::Mosquito && b.gt.p && own_q != NONE && dist(c, own_q) == 1 {
                        f[opp as usize][49] = 1;
                    }
                }
                continue;
            }
            if !own_queen {
                continue;
            }
            let free = b.lvl[p as usize] > 0 || (pinned >> p) & 1 == 0;
            if free && p != b.last {
                if k == 0 {
                    if can_slide(b, c) {
                        f[ci][5] += 1;
                    }
                } else {
                    let slider = matches!(KIND_BUG[k], Bug::Spider | Bug::Ant | Bug::Pillbug) && b.lvl[p as usize] == 0;
                    if !slider || can_slide(b, c) {
                        f[ci][6 + bug - 1] += 1;
                        movable[ci] += 1;
                        if KIND_BUG[k] == Bug::Mosquito && b.lvl[p as usize] == 0 && can_slide(b, c) && ant_power(b, c) {
                            f[ci][55] += 1;
                        }
                    }
                }
            }
        }
        if qpos != NONE {
            let qi = qpos as usize;
            let mut occ = 0i32;
            let mut escape = 0i32;
            for d in 0..6 {
                let n = t.nb[qi][d] as usize;
                if b.hgt[n] == 0 {
                    continue;
                }
                occ += 1;
                let tp = b.top[n];
                let tfree = b.hgt[n] > 1 || (pinned >> tp) & 1 == 0;
                if color_of(tp) == color {
                    if !tfree && b.hgt[n] == 1 {
                        f[ci][50] += 1;
                    }
                    if tfree && bug_of(tp) != Bug::Queen {
                        escape += 1;
                        if matches!(bug_of(tp), Bug::Beetle | Bug::Grasshopper | Bug::Ladybug | Bug::Mosquito) {
                            f[ci][47] += 1;
                        }
                    }
                    if b.gt.p && b.hgt[n] == 1
                        && (bug_of(tp) == Bug::Pillbug || (bug_of(tp) == Bug::Mosquito && pillbug_power(b, n as u16)))
                    {
                        f[ci][23] = 1;
                    }
                } else {
                    if !tfree {
                        f[ci][24] += 1;
                    }
                    let tb = bug_of(tp);
                    if tfree && (tb == Bug::Beetle || (tb == Bug::Mosquito && (b.hgt[n] > 1 || beetle_power(b, n as u16)))) && tp != b.last {
                        f[ci][35] += 1;
                    }
                }
            }
            if !can_slide(b, qpos) && occ >= 2 {
                f[ci][36] = 1;
            }
            // where the enemy could drop a beetle/mosquito close to our queen
            {
                let eb = (1 - ci) * 14;
                let has_b = [3usize, 4, 11].iter().any(|&k2| b.gt.includes_kind(k2) && b.pos[eb + k2] == NONE);
                if has_b {
                    f[ci][54] = drop_spots_near(b, (1 - ci) as u8, qpos).min(3);
                }
            }
            // enemy beetles one step away from becoming a threat next to our queen
            for k2 in [3usize, 4, 11] {
                if !b.gt.includes_kind(k2) {
                    continue;
                }
                let ep = ((1 - ci) * 14 + k2) as u8;
                let ec = b.pos[ep as usize];
                if ec == NONE || b.top[ec as usize] != ep || ep == b.last {
                    continue;
                }
                if dist(ec, qpos) != 2 {
                    continue;
                }
                let efree = b.lvl[ep as usize] > 0 || (pinned >> ep) & 1 == 0;
                if !efree {
                    continue;
                }
                let is_beetle = KIND_BUG[k2] == Bug::Beetle;
                if is_beetle || b.lvl[ep as usize] > 0 || beetle_power(b, ec) {
                    f[ci][53] += 1;
                }
            }
            if occ >= 1 && occ <= 5 {
                let wt = weights();
                let access = if occ >= 2 && wt[51] == 0 && wt[52] == 0 {
                    RING_BUF.with(|rb| b.ring_access(opp, &mut rb.borrow_mut()))
                } else if occ >= 2 {
                    let (cells, pieces) = RING_BUF.with(|rb| b.ring_fill(opp, &mut rb.borrow_mut()));
                    f[opp as usize][51] = cells as i32;
                    f[opp as usize][52] = pieces.min(3) as i32;
                    cells > 0
                } else {
                    true
                };
                if access {
                    f[ci][(occ - 1) as usize] = 1;
                } else {
                    f[ci][37 + (occ - 1) as usize] = 1;
                }
            }
            if occ >= 6 {
                f[ci][23] = 0;
            }
            if b.gt.p && b.pos[base + 13] == NONE && occ < 6 && b.ply >= 2 {
                let mut placeable = false;
                for d in 0..6 {
                    let e = t.nb[qi][d] as usize;
                    if b.hgt[e] != 0 {
                        continue;
                    }
                    let mut ok = true;
                    for d2 in 0..6 {
                        let g = t.nb[e][d2] as usize;
                        if b.hgt[g] != 0 && color_of(b.top[g]) != color {
                            ok = false;
                            break;
                        }
                    }
                    if ok {
                        placeable = true;
                        break;
                    }
                }
                f[ci][45] = placeable as i32;
            }
            if occ >= 2 {
                let (rem, esc) = b.relocation_defense(color);
                f[ci][43] = rem;
                f[ci][44] = esc as i32;
            }
            f[ci][25] = if occ >= 3 { escape * occ } else { 0 };
            if b.hgt[qi] > 1 && color_of(b.top[qi]) == opp {
                f[opp as usize][21] += 1;
            }
        } else if b.turns_taken(color) >= 2 {
            f[ci][34] = 1;
        }
    }
    // attack strength vs vacancies around the enemy queen (Ingersoll's "strength")
    for ci in 0..2usize {
        let eq = b.pos[(1 - ci) * 14];
        if eq != NONE && b.queen_placed(ci as u8) {
            let vac = (0..6).filter(|&d| b.hgt[t.nb[eq as usize][d] as usize] == 0).count() as i32;
            let avail = movable[ci] + in_hand[ci];
            f[ci][46] = (avail - vac).clamp(-4, 0);
            if in_hand[ci] > 0 {
                let mut dd = 0;
                for d in 0..6 {
                    let e = t.nb[eq as usize][d] as usize;
                    if b.hgt[e] != 0 {
                        continue;
                    }
                    let ok = (0..6).all(|d2| {
                        let g = t.nb[e][d2] as usize;
                        b.hgt[g] == 0 || color_of(b.top[g]) == ci as u8
                    });
                    if ok {
                        dd += 1;
                    }
                }
                f[ci][48] = dd.min(2);
            }
        }
        // our beetle on top of the enemy pillbug
        for k in 0..14 {
            let p = ((1 - ci) * 14 + k) as u8;
            if KIND_BUG[k] == Bug::Pillbug && b.gt.includes_kind(k) {
                let c = b.pos[p as usize];
                if c != NONE && b.top[c as usize] != p && color_of(b.top[c as usize]) == ci as u8 {
                    f[ci][49] = 1;
                }
            }
        }
    }
    if b.ply >= 6 {
        for ci in 0..2usize {
            let spots = if in_hand[ci] > 0 { placement_spots(b, ci as u8, 6) } else { 0 };
            let freedom = movable[ci] + spots.min(3);
            f[ci][28] = spots.min(4);
            if freedom <= 4 {
                f[ci][29 + freedom as usize] = 1;
            }
        }
    }
    f
}

/// Count empty cells where `color` could place (up to `cap`).
pub fn placement_spots(b: &Board, color: u8, cap: i32) -> i32 {
    let t = tables();
    let mut seen = [0u16; 64];
    let mut n = 0usize;
    let mut count = 0;
    for q in 0..28u8 {
        let c = b.pos[q as usize];
        if c == NONE || b.top[c as usize] != q || color_of(q) != color {
            continue;
        }
        for d in 0..6 {
            let e = t.nb[c as usize][d];
            if b.hgt[e as usize] != 0 || seen[..n].contains(&e) {
                continue;
            }
            if n < 64 {
                seen[n] = e;
                n += 1;
            }
            let mut ok = true;
            for d2 in 0..6 {
                let f = t.nb[e as usize][d2] as usize;
                if b.hgt[f] != 0 && color_of(b.top[f]) != color {
                    ok = false;
                    break;
                }
            }
            if ok {
                count += 1;
                if count >= cap {
                    return count;
                }
            }
        }
    }
    count
}

pub fn simple_mode() -> bool {
    use std::sync::OnceLock;
    static M: OnceLock<bool> = OnceLock::new();
    *M.get_or_init(|| std::env::var("PROPOLIS_EVAL").map(|v| v == "simple").unwrap_or(false))
}

/// Foulbrood/HotFrass baseline evaluation (for A/B testing).
pub fn evaluate_simple(b: &Board) -> i32 {
    let t = tables();
    let mut sc = [0i32; 2];
    let qs = [b.pos[0], b.pos[14]];
    for color in 0..2usize {
        let q = qs[color];
        if q != NONE {
            let mut occ = 0;
            for d in 0..6 {
                if b.hgt[t.nb[q as usize][d] as usize] != 0 {
                    occ += 1;
                }
            }
            sc[color] -= [0, 20, 55, 110, 220, 450, 0][occ];
            if b.top[q as usize] != (color * 14) as u8 {
                sc[color] -= 80;
            }
        }
    }
    for p in 0..28u8 {
        let c = b.pos[p as usize];
        if c == NONE || !b.gt.includes_kind((p % 14) as usize) {
            continue;
        }
        let col = color_of(p) as usize;
        sc[col] += 12;
        if b.top[c as usize] != p {
            sc[col] -= 15;
        }
        if p % 14 != 0 {
            let eq = qs[1 - col];
            if eq != NONE {
                sc[col] += 3 * (6 - dist(c, eq).min(6));
            }
        }
    }
    let side = b.side() as usize;
    sc[side] - sc[1 - side]
}

/// Does a ground piece at `c` have at least one freedom-to-move slide?
#[inline(always)]
/// Number of `color`'s legal placement cells within distance 2 of `q`.
pub fn drop_spots_near(b: &Board, color: u8, q: u16) -> i32 {
    let t = tables();
    let mut cells = [0u16; 19];
    let mut n = 0usize;
    for &a in t.nb[q as usize].iter() {
        if !cells[..n].contains(&a) { cells[n] = a; n += 1; }
        for &c in t.nb[a as usize].iter() {
            if c != q && !cells[..n].contains(&c) && n < 19 { cells[n] = c; n += 1; }
        }
    }
    let mut cnt = 0;
    for &e in cells[..n].iter() {
        if b.hgt[e as usize] != 0 {
            continue;
        }
        let mut touch = false;
        let mut ok = true;
        for &f in t.nb[e as usize].iter() {
            if b.hgt[f as usize] != 0 {
                touch = true;
                if color_of(b.top[f as usize]) != color {
                    ok = false;
                    break;
                }
            }
        }
        if ok && touch {
            cnt += 1;
        }
    }
    cnt
}

/// A ground mosquito at `c` can move like an ant when it touches an ant.
pub fn ant_power(b: &Board, c: u16) -> bool {
    let t = tables();
    t.nb[c as usize].iter().any(|&n| b.hgt[n as usize] > 0 && bug_of(b.top[n as usize]) == Bug::Ant)
}

/// A ground mosquito at `c` gains the pillbug's ability when it touches a pillbug.
pub fn pillbug_power(b: &Board, c: u16) -> bool {
    let t = tables();
    t.nb[c as usize].iter().any(|&n| b.hgt[n as usize] > 0 && bug_of(b.top[n as usize]) == Bug::Pillbug)
}

/// A ground mosquito at `c` copies a beetle when any neighbouring stack top is a beetle.
pub fn beetle_power(b: &Board, c: u16) -> bool {
    let t = tables();
    t.nb[c as usize].iter().any(|&n| b.hgt[n as usize] > 0 && bug_of(b.top[n as usize]) == Bug::Beetle)
}

pub fn can_slide(b: &Board, c: u16) -> bool {
    let t = tables();
    let nb = &t.nb[c as usize];
    for d in 0..6 {
        if b.hgt[nb[d] as usize] == 0 {
            let l = b.hgt[nb[(d + 5) % 6] as usize] != 0;
            let r = b.hgt[nb[(d + 1) % 6] as usize] != 0;
            if l ^ r {
                return true;
            }
        }
    }
    false
}

/// Small MLP evaluation loaded from `TR_NET` (text): inputs are the side-to-move's
/// features followed by the opponent's, normalised by stored mean/std.
pub struct Net {
    n_in: usize,
    h1: usize,
    h2: usize,
    mean: Vec<f32>,
    inv_std: Vec<f32>,
    w1: Vec<f32>,
    b1: Vec<f32>,
    w2: Vec<f32>,
    b2: Vec<f32>,
    w3: Vec<f32>,
    b3: f32,
    scale: f32,
}

impl Net {
    fn load(path: &str) -> Option<Net> {
        let txt = std::fs::read_to_string(path).ok()?;
        let mut it = txt.split_whitespace();
        let mut num = || -> Option<f32> { it.next()?.parse::<f32>().ok() };
        let n_in = num()? as usize;
        let h1 = num()? as usize;
        let h2 = num()? as usize;
        let mut take = |n: usize, num: &mut dyn FnMut() -> Option<f32>| -> Option<Vec<f32>> { (0..n).map(|_| num()).collect() };
        let mean = take(n_in, &mut num)?;
        let std = take(n_in, &mut num)?;
        let w1 = take(n_in * h1, &mut num)?;
        let b1 = take(h1, &mut num)?;
        let w2 = take(h1 * h2, &mut num)?;
        let b2 = take(h2, &mut num)?;
        let w3 = take(h2, &mut num)?;
        let b3 = num()?;
        let scale = num()?;
        if n_in != 2 * (NF - 1) && n_in != 2 * (NF - 1) + EXTRA {
            eprintln!("TR_NET: expected {} or {} inputs, file has {}", 2 * (NF - 1), 2 * (NF - 1) + EXTRA, n_in);
            return None;
        }
        let inv_std = std.iter().map(|s| if *s > 1e-6 { 1.0 / s } else { 0.0 }).collect();
        Some(Net { n_in, h1, h2, mean, inv_std, w1, b1, w2, b2, w3, b3, scale })
    }

    pub fn eval(&self, me: &[i32; NF], opp: &[i32; NF], ex: &[f32; EXTRA]) -> i32 {
        let mut x = [0f32; 2 * NF + EXTRA];
        for i in 0..NF - 1 {
            x[i] = me[i] as f32;
            x[NF - 1 + i] = opp[i] as f32;
        }
        if self.n_in > 2 * (NF - 1) {
            x[2 * (NF - 1)..2 * (NF - 1) + EXTRA].copy_from_slice(ex);
        }
        for i in 0..self.n_in {
            x[i] = (x[i] - self.mean[i]) * self.inv_std[i];
        }
        let mut a1 = [0f32; 256];
        for j in 0..self.h1 {
            let row = &self.w1[j * self.n_in..(j + 1) * self.n_in];
            let mut s = self.b1[j];
            for i in 0..self.n_in {
                s += row[i] * x[i];
            }
            a1[j] = s.clamp(0.0, 8.0);
        }
        let mut a2 = [0f32; 256];
        for j in 0..self.h2 {
            let row = &self.w2[j * self.h1..(j + 1) * self.h1];
            let mut s = self.b2[j];
            for i in 0..self.h1 {
                s += row[i] * a1[i];
            }
            a2[j] = s.clamp(0.0, 8.0);
        }
        let mut out = self.b3;
        for j in 0..self.h2 {
            out += self.w3[j] * a2[j];
        }
        (out * self.scale).clamp(-20000.0, 20000.0) as i32
    }
}

pub fn net() -> Option<&'static Net> {
    use std::sync::OnceLock;
    static N: OnceLock<Option<Net>> = OnceLock::new();
    N.get_or_init(|| {
        let path = match std::env::var("TR_NET") {
            Ok(p) => p,
            Err(_) => std::env::current_exe().ok()?.parent()?.join("thoraxrex-net.txt").to_string_lossy().into_owned(),
        };
        if path.is_empty() || !std::path::Path::new(&path).exists() {
            return None;
        }
        Net::load(&path)
    })
    .as_ref()
}

/// Extra network inputs: for each of the 28 piece slots (side to move first), whether it is on
/// the board, its distance to its own queen and to the enemy queen (clipped, /4), whether it
/// is on top of its stack, and whether it is pinned.
pub const EXTRA: usize = 28 * 5;

pub fn extra_inputs(b: &Board, out: &mut [f32]) {
    let me = b.side() as usize;
    let pinned = b.pinned_mask();
    for slot in 0..28usize {
        let side = if slot < 14 { me } else { 1 - me };
        let k = slot % 14;
        let p = (side * 14 + k) as u8;
        let o = &mut out[slot * 5..slot * 5 + 5];
        o.iter_mut().for_each(|v| *v = 0.0);
        if !b.gt.includes_kind(k) {
            continue;
        }
        let c = b.pos[p as usize];
        if c == NONE {
            continue;
        }
        o[0] = 1.0;
        let oq = b.pos[side * 14];
        let eq = b.pos[(1 - side) * 14];
        o[1] = if oq != NONE { (dist(c, oq).min(4) as f32) / 4.0 } else { 1.0 };
        o[2] = if eq != NONE { (dist(c, eq).min(4) as f32) / 4.0 } else { 1.0 };
        o[3] = if b.top[c as usize] == p { 1.0 } else { 0.0 };
        o[4] = if (pinned >> p) & 1 != 0 { 1.0 } else { 0.0 };
    }
}

/// Hand-set terms added on top of the net (TR_HX="mob,lowmob,threat,pinnedguard,beetlethreat,pbthreat,pbcovered").
fn hand_w() -> [i32; 7] {
    static W: std::sync::OnceLock<[i32; 7]> = std::sync::OnceLock::new();
    *W.get_or_init(|| {
        let mut w = [8, 60, 25, 25, 20, 200, 240];
        if let Ok(v) = std::env::var("TR_HX") {
            for (i, t) in v.split(',').enumerate().take(7) {
                if let Ok(x) = t.trim().parse() {
                    w[i] = x;
                }
            }
        }
        w
    })
}

fn hand_terms(b: &Board, fb: &[[i32; NF]; 2], me: usize, w: &[i32; 7]) -> i32 {
    let t = tables();
    let pinned = b.pinned_mask();
    let mut side = [0i32; 2];
    for c in 0..2usize {
        let mob: i32 = fb[c][5..13].iter().sum();
        let on_board = fb[c][26];
        side[c] += w[0] * mob.min(8);
        if b.queen_placed(c as u8) && on_board >= 3 && mob <= 1 {
            side[c] -= w[1];
        }
        // our beetles that an enemy mosquito (copying the beetle) or beetle could climb onto and pin
        for k in [3usize, 4] {
            let p = (c * 14 + k) as u8;
            let bc = b.pos[p as usize];
            if bc == NONE || b.top[bc as usize] != p || b.hgt[bc as usize] != 1 {
                continue;
            }
            let mut hit = false;
            for d2 in 0..6 {
                let n2 = t.nb[bc as usize][d2] as usize;
                if b.hgt[n2] == 0 {
                    continue;
                }
                let e = b.top[n2];
                if color_of(e) == c as u8 {
                    continue;
                }
                let free = b.hgt[n2] > 1 || (pinned >> e) & 1 == 0;
                if free && matches!(bug_of(e), Bug::Mosquito | Bug::Beetle) {
                    hit = true;
                }
            }
            if hit {
                side[c] -= w[4];
            }
        }
        let q = b.pos[c * 14];
        if q == NONE {
            continue;
        }
        let mut threatened = 0;
        let mut pinned_guard = 0;
        for d in 0..6 {
            let cell = t.nb[q as usize][d] as usize;
            if b.hgt[cell] == 0 {
                continue;
            }
            let tp = b.top[cell];
            if color_of(tp) != c as u8 || bug_of(tp) == Bug::Queen {
                continue;
            }
            if b.hgt[cell] == 1 && (pinned >> tp) & 1 != 0 && matches!(bug_of(tp), Bug::Beetle | Bug::Mosquito) {
                pinned_guard += 1;
            }
            // an enemy climber next to this guard could cover it
            let mut hit = false;
            for d2 in 0..6 {
                let n2 = t.nb[cell][d2] as usize;
                if b.hgt[n2] == 0 || n2 == q as usize {
                    continue;
                }
                let e = b.top[n2];
                if color_of(e) == c as u8 {
                    continue;
                }
                let free = b.hgt[n2] > 1 || (pinned >> e) & 1 == 0;
                let climber = b.hgt[n2] > 1 || bug_of(e) == Bug::Beetle || (bug_of(e) == Bug::Mosquito && beetle_power(b, n2 as u16));
                if free && climber {
                    hit = true;
                }
            }
            if hit {
                threatened += 1;
                // our queen's pillbug guard (pillbug, or mosquito copying one) about to be covered
                if bug_of(tp) == Bug::Pillbug || (bug_of(tp) == Bug::Mosquito && b.hgt[cell] == 1 && pillbug_power(b, cell as u16)) {
                    side[c] -= w[5];
                }
            }
        }
        // our pillbug or mosquito next to our queen already covered by the enemy
        for d in 0..6 {
            let cell = t.nb[q as usize][d] as usize;
            if b.hgt[cell] < 2 || color_of(b.top[cell]) == c as u8 {
                continue;
            }
            let Some(base) = (0..28u8).find(|&p| b.pos[p as usize] as usize == cell && b.lvl[p as usize] == 0) else { continue };
            if color_of(base) == c as u8 && matches!(bug_of(base), Bug::Pillbug | Bug::Mosquito) {
                side[c] -= w[6];
            }
        }
        side[c] -= w[2] * threatened + w[3] * pinned_guard;
    }
    side[me] - side[1 - me]
}

/// How exposed `c`'s queen is: occupied neighbours and enemy pieces that could climb next to it.
fn queen_danger(b: &Board, c: usize) -> i32 {
    let t = tables();
    let q = b.pos[c * 14];
    if q == NONE {
        return 0;
    }
    let mut occ = 0usize;
    let mut climbers = 0i32;
    for d in 0..6 {
        let n = t.nb[q as usize][d] as usize;
        if b.hgt[n] == 0 {
            continue;
        }
        occ += 1;
        let e = b.top[n];
        if color_of(e) != c as u8 && (b.hgt[n] > 1 || matches!(bug_of(e), Bug::Beetle | Bug::Mosquito)) {
            climbers += 1;
        }
    }
    if b.hgt[q as usize] > 1 && color_of(b.top[q as usize]) != c as u8 {
        climbers += 2;
    }
    [0, 0, 15, 40, 90, 200, 400][occ] + 40 * climbers
}

fn qd_far() -> i32 {
    static D: std::sync::OnceLock<i32> = std::sync::OnceLock::new();
    *D.get_or_init(|| std::env::var("TR_QDFAR").ok().and_then(|v| v.parse().ok()).unwrap_or(0))
}
fn qd_cov() -> (i32, i32) {
    static D: std::sync::OnceLock<(i32, i32)> = std::sync::OnceLock::new();
    *D.get_or_init(|| {
        let v = std::env::var("TR_QCOV").unwrap_or_default();
        let mut it = v.split(',').filter_map(|t| t.trim().parse().ok());
        (it.next().unwrap_or(4), it.next().unwrap_or(0))
    })
}
fn qon_w() -> i32 {
    static D: std::sync::OnceLock<i32> = std::sync::OnceLock::new();
    *D.get_or_init(|| std::env::var("TR_QON").ok().and_then(|v| v.parse().ok()).unwrap_or(150))
}
fn qd_sg() -> i32 {
    static D: std::sync::OnceLock<i32> = std::sync::OnceLock::new();
    *D.get_or_init(|| std::env::var("TR_QSG").ok().and_then(|v| v.parse().ok()).unwrap_or(0))
}
fn pbq_w() -> i32 {
    static D: std::sync::OnceLock<i32> = std::sync::OnceLock::new();
    *D.get_or_init(|| std::env::var("TR_PBQ").ok().and_then(|v| v.parse().ok()).unwrap_or(0))
}
/// 1 when colour c's queen and pillbug are both on the board but the pillbug can't guard the queen:
/// not adjacent, or covered (by either side's piece). 0 otherwise.
fn pb_away(b: &Board, c: usize) -> i32 {
    let q = b.pos[c * 14];
    let p = (c * 14 + 13) as u8;
    let pc = b.pos[p as usize];
    if q == NONE || pc == NONE || !b.gt.includes_kind(13) {
        return 0;
    }
    if dist(pc, q) != 1 || b.top[pc as usize] != p {
        return 1;
    }
    0
}
fn qona_w() -> i32 {
    static D: std::sync::OnceLock<i32> = std::sync::OnceLock::new();
    *D.get_or_init(|| std::env::var("TR_QONA").ok().and_then(|v| v.parse().ok()).unwrap_or_else(qon_w))
}
fn early_w() -> (i32, i32) {
    static D: std::sync::OnceLock<(i32, i32)> = std::sync::OnceLock::new();
    *D.get_or_init(|| {
        let v = std::env::var("TR_EARLY").unwrap_or_default();
        let mut it = v.split(',').filter_map(|t| t.trim().parse().ok());
        (it.next().unwrap_or(0), it.next().unwrap_or(8))
    })
}
fn qd_scale() -> i32 {
    static D: std::sync::OnceLock<i32> = std::sync::OnceLock::new();
    *D.get_or_init(|| std::env::var("TR_QD").ok().and_then(|v| v.parse().ok()).unwrap_or(20))
}

/// Nonlinear queen danger for colour `c` (chess-style king danger): attack units minus defence units,
/// squared. Returns raw units^2 (scaled by TR_QD/10 at the call site), capped.
fn qdanger(b: &Board, c: usize) -> i32 {
    let t = tables();
    let q = b.pos[c * 14];
    if q == NONE {
        return 0;
    }
    let e = 1 - c;
    let pinned = b.pinned_mask();
    let mut att = 0i32;
    let mut def = 0i32;
    let mut occ = 0;
    let mut empty_nb = 0;
    for d in 0..6 {
        let n = t.nb[q as usize][d] as usize;
        if b.hgt[n] == 0 {
            empty_nb += 1;
            continue;
        }
        occ += 1;
        let tp = b.top[n];
        let free = b.hgt[n] > 1 || (pinned >> tp) & 1 == 0;
        if color_of(tp) as usize == e {
            let climber = b.hgt[n] > 1 || bug_of(tp) == Bug::Beetle || (bug_of(tp) == Bug::Mosquito && beetle_power(b, n as u16));
            if climber && free {
                att += 3;
            } else if !free {
                att += 1;
            }
        } else {
            if bug_of(tp) == Bug::Pillbug || (bug_of(tp) == Bug::Mosquito && b.hgt[n] == 1 && pillbug_power(b, n as u16)) {
                if free || bug_of(tp) == Bug::Pillbug {
                    def += 3;
                }
            } else if free && bug_of(tp) == Bug::Beetle {
                def += 1;
            }
            if !free && b.hgt[n] == 1 {
                att += 1;
            }
        }
    }
    att += occ;
    let (qcov, pbcov) = qd_cov();
    if b.hgt[q as usize] > 1 && color_of(b.top[q as usize]) as usize == e {
        att += qcov;
    }
    if pbcov != 0 {
        // our pillbug (or mosquito) next to the queen buried under an enemy piece: the queen's guard is gone
        for d in 0..6 {
            let n = t.nb[q as usize][d] as usize;
            if b.hgt[n] < 2 || color_of(b.top[n]) as usize != e {
                continue;
            }
            if let Some(base) = (0..28u8).find(|&p| b.pos[p as usize] as usize == n && b.lvl[p as usize] == 0) {
                if color_of(base) as usize == c && matches!(bug_of(base), Bug::Pillbug | Bug::Mosquito) {
                    att += pbcov;
                }
            }
        }
    }
    // enemy climbers two steps away
    for k in [3usize, 4, 11] {
        if !b.gt.includes_kind(k) {
            continue;
        }
        let p = (e * 14 + k) as u8;
        let pc = b.pos[p as usize];
        if pc == NONE || b.top[pc as usize] != p {
            continue;
        }
        let dd = dist(pc, q);
        if (dd == 2 || dd == 3) && (b.hgt[pc as usize] > 1 || (pinned >> p) & 1 == 0) {
            if k != 11 || b.hgt[pc as usize] > 1 || beetle_power(b, pc) {
                att += if dd == 2 { 2 } else { qd_far() };
            }
        }
    }
    // enemy free ants (reach any empty neighbour)
    if empty_nb > 0 {
        let mut ants = 0;
        for k in [8usize, 9, 10] {
            let p = (e * 14 + k) as u8;
            let pc = b.pos[p as usize];
            if pc != NONE && b.top[pc as usize] == p && (pinned >> p) & 1 == 0 && dist(pc, q) > 1 {
                ants += 1;
            }
        }
        att += ants.min(empty_nb).min(2);
    }
    // enemy free spiders / grasshoppers close by: slow fillers of the queen's last spaces
    let qsg = qd_sg();
    if qsg != 0 && empty_nb > 0 {
        let mut n_sg = 0;
        for k in [1usize, 2, 5, 6, 7, 12] {
            if !b.gt.includes_kind(k) { continue; }
            let p = (e * 14 + k) as u8;
            let pc = b.pos[p as usize];
            if pc != NONE && b.top[pc as usize] == p && (pinned >> p) & 1 == 0 {
                let dd = dist(pc, q);
                if dd >= 2 && dd <= 3 { n_sg += 1; }
            }
        }
        att += qsg * n_sg.min(empty_nb).min(2);
    }
    att += 2 * drop_spots_near(b, e as u8, q).min(2);
    let u = (att - def).max(0);
    (u * u).min(400)
}

fn gb_w() -> (i32, i32) {
    static D: std::sync::OnceLock<(i32, i32)> = std::sync::OnceLock::new();
    *D.get_or_init(|| {
        let v = std::env::var("TR_GB").unwrap_or_else(|_| "250,60".to_string());
        let mut it = v.split(',').filter_map(|t| t.trim().parse().ok());
        (it.next().unwrap_or(0), it.next().unwrap_or(0))
    })
}

/// Value of colour `c`'s beetles (and beetle-powered mosquito) being grounded (on the floor and pinned):
/// returns a penalty-sized number for `c` = sum over its grounded climbers, bigger when near the enemy queen.
fn grounded_beetles(b: &Board, c: usize, near: i32, far: i32) -> i32 {
    let pinned = b.pinned_mask();
    let eq = b.pos[(1 - c) * 14];
    let mut s = 0;
    for k in [3usize, 4, 11] {
        let p = (c * 14 + k) as u8;
        let pc = b.pos[p as usize];
        if pc == NONE || b.top[pc as usize] != p || b.hgt[pc as usize] != 1 || (pinned >> p) & 1 == 0 {
            continue;
        }
        if k == 11 && !beetle_power(b, pc) {
            continue;
        }
        s += if eq != NONE && dist(pc, eq) <= 3 { near } else { far };
    }
    s
}

fn fort_w() -> i32 {
    static D: std::sync::OnceLock<i32> = std::sync::OnceLock::new();
    *D.get_or_init(|| std::env::var("TR_FORT").ok().and_then(|v| v.parse().ok()).unwrap_or(0))
}

/// 1 when colour `c` has a defensive "fortress": its pillbug (or a pillbug-copying mosquito) sits free next
/// to its queen, and every enemy climber on the board (beetles, beetle-capable mosquito) is either grounded
/// (on the floor and pinned) or more than 3 steps from c's queen. 0 otherwise.
fn fortress(b: &Board, c: usize) -> i32 {
    let t = tables();
    let q = b.pos[c * 14];
    if q == NONE {
        return 0;
    }
    let pinned = b.pinned_mask();
    let mut guard = false;
    for d in 0..6 {
        let n = t.nb[q as usize][d] as usize;
        if b.hgt[n] != 1 {
            continue;
        }
        let tp = b.top[n];
        if color_of(tp) as usize != c {
            continue;
        }
        let pb = bug_of(tp) == Bug::Pillbug || (bug_of(tp) == Bug::Mosquito && pillbug_power(b, n as u16));
        if pb && (bug_of(tp) == Bug::Pillbug || (pinned >> tp) & 1 == 0) {
            guard = true;
        }
    }
    if !guard {
        return 0;
    }
    let e = 1 - c;
    for k in [3usize, 4, 11] {
        let p = (e * 14 + k) as u8;
        let pc = b.pos[p as usize];
        if pc == NONE {
            continue;
        }
        if b.top[pc as usize] != p {
            continue; // covered: not a threat right now
        }
        let climber = k != 11 || b.hgt[pc as usize] > 1 || beetle_power(b, pc);
        if !climber {
            continue;
        }
        let grounded = b.hgt[pc as usize] == 1 && (pinned >> p) & 1 != 0;
        if !grounded && dist(pc, q) <= 3 {
            return 0;
        }
    }
    1
}
