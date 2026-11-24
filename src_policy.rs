//! Move-policy network: a small transformer over the pieces on the board.
//!
//! Tokens: one per placed piece (stacked pieces included) plus one global token (pieces in hand, phase).
//! Attention gets a learned per-head bias by hex distance between pieces (bucket 0..7, 8 = global token),
//! so the network sees the hive's geometry without absolute coordinates (Hive has no fixed board).
//! Each legal move is scored from the moving piece's output vector (or an in-hand embedding for placements),
//! the mean output vector of the pieces around the destination, and a few move flags.
//!
//! Training lives in tools/policy_train.py (PyTorch); this file holds the feature encoder (shared with
//! training via `thoraxrex policydump`) and a plain-Rust forward pass for use inside the search.

use crate::board::*;
use std::io::Write;

pub const TF: usize = 32; // token features
pub const MF: usize = 12; // move flags
pub const NB: usize = 7; // destination neighbour slots (6 neighbours + the piece under a climb)
pub const DB: usize = 9; // distance buckets: 0..7 (7 = 7+), 8 = involves the global token

pub struct Encoded {
    pub tok: Vec<[f32; TF]>,
    pub cell: Vec<u16>, // NONE for the global token (last)
    pub tok_of_piece: [i16; 28],
}

#[inline]
fn wrap(d: i32) -> i32 {
    ((d + 16) & 31) - 16
}

pub fn hexdist(a: u16, b: u16) -> i32 {
    let (aq, ar) = ((a & 31) as i32, ((a >> 5) & 31) as i32);
    let (bq, br) = ((b & 31) as i32, ((b >> 5) & 31) as i32);
    let dq = wrap(aq - bq);
    let dr = wrap(ar - br);
    (dq.abs() + dr.abs() + (dq + dr).abs()) / 2
}

fn adjacent(a: u16, b: u16) -> bool {
    let t = tables();
    (0..6).any(|d| t.nb[a as usize][d] == b)
}

/// Encode the position from the side to move's point of view.
pub fn encode(b: &Board) -> Encoded {
    let t = tables();
    let me = b.side() as usize;
    let pinned = b.pinned_mask();
    let qown = b.pos[me * 14];
    let qopp = b.pos[(1 - me) * 14];
    let mut tok = Vec::with_capacity(29);
    let mut cell = Vec::with_capacity(29);
    let mut top = [NONE; 28];
    let mut tok_of_piece = [-1i16; 28];
    for p in 0..28u8 {
        let c = b.pos[p as usize];
        if c == NONE {
            continue;
        }
        let mut f = [0f32; TF];
        let own = color_of(p) as usize == me;
        f[(if own { 0 } else { 8 }) + bug_of(p) as usize] = 1.0;
        let lvl = b.lvl[p as usize] as usize;
        f[17 + lvl.min(3)] = 1.0;
        let is_top = b.top[c as usize] == p;
        f[21] = is_top as u8 as f32;
        f[22] = (is_top && (pinned >> p) & 1 == 1) as u8 as f32;
        f[23] = (b.last == p) as u8 as f32;
        let (mut on, mut opn) = (0, 0);
        for d in 0..6 {
            let n = t.nb[c as usize][d] as usize;
            if b.hgt[n] > 0 {
                if color_of(b.top[n]) as usize == me { on += 1 } else { opn += 1 }
            }
        }
        f[24] = on as f32 / 6.0;
        f[25] = opn as f32 / 6.0;
        f[26] = (qown != NONE && adjacent(c, qown)) as u8 as f32;
        f[27] = (qopp != NONE && adjacent(c, qopp)) as u8 as f32;
        f[28] = if qown == NONE { 1.0 } else { (hexdist(c, qown) as f32 / 8.0).min(1.0) };
        f[29] = if qopp == NONE { 1.0 } else { (hexdist(c, qopp) as f32 / 8.0).min(1.0) };
        if !is_top {
            let tp = b.top[c as usize];
            if color_of(tp) as usize == me { f[30] = 1.0 } else { f[31] = 1.0 }
        }
        tok_of_piece[p as usize] = tok.len() as i16;
        top[p as usize] = c;
        tok.push(f);
        cell.push(c);
    }
    // global token: pieces in hand (own 0..7, opp 8..15), flag, phase, queens placed
    let mut g = [0f32; TF];
    for p in 0..28u8 {
        if !b.gt.includes_kind((p % 14) as usize) || b.pos[p as usize] != NONE {
            continue;
        }
        let own = color_of(p) as usize == me;
        g[(if own { 0 } else { 8 }) + bug_of(p) as usize] += 1.0 / 3.0;
    }
    g[16] = 1.0;
    g[17] = (b.ply as f32 / 100.0).min(1.0);
    g[18] = (qown != NONE) as u8 as f32;
    g[19] = (qopp != NONE) as u8 as f32;
    tok.push(g);
    cell.push(NONE);
    let _ = top;
    Encoded { tok, cell, tok_of_piece }
}

pub struct MoveFeat {
    pub piece_tok: i16, // token index, or -(1 + bug) for a placement
    pub nb: [i16; NB],  // -1 = empty slot
    pub f: [f32; MF],
}

pub fn move_features(b: &Board, e: &Encoded, m: Move) -> MoveFeat {
    let t = tables();
    let me = b.side() as usize;
    let qown = b.pos[me * 14];
    let qopp = b.pos[(1 - me) * 14];
    let p = m.piece();
    let to = m.to();
    let mut f = [0f32; MF];
    let piece_tok = if m.is_place() { -(1 + bug_of(p) as i16) } else { e.tok_of_piece[p as usize] };
    f[0] = m.is_place() as u8 as f32;
    f[1] = (color_of(p) as usize != me) as u8 as f32;
    // height at destination (excluding the mover if it is already there, which cannot happen for a move)
    f[2] = (b.hgt[to as usize] as f32 / 3.0).min(1.0);
    if !m.is_place() {
        let fr = m.from();
        f[3] = (b.hgt[fr as usize] > 1) as u8 as f32;
        f[6] = (qown != NONE && adjacent(fr, qown)) as u8 as f32;
        f[7] = (qopp != NONE && adjacent(fr, qopp)) as u8 as f32;
    }
    f[4] = (qown != NONE && adjacent(to, qown)) as u8 as f32;
    f[5] = (qopp != NONE && adjacent(to, qopp)) as u8 as f32;
    let mut nb = [-1i16; NB];
    let (mut on, mut opn) = (0, 0);
    for d in 0..6 {
        let n = t.nb[to as usize][d] as usize;
        if b.hgt[n] == 0 {
            continue;
        }
        let mut tp = b.top[n];
        if !m.is_place() && tp == p {
            // the mover leaves this cell: the piece beneath (if any) becomes the neighbour
            if b.hgt[n] < 2 {
                continue;
            }
            tp = b.under[p as usize];
            if tp == NOP {
                continue;
            }
        }
        nb[d] = e.tok_of_piece[tp as usize];
        if color_of(tp) as usize == me { on += 1 } else { opn += 1 }
    }
    if b.hgt[to as usize] > 0 {
        nb[6] = e.tok_of_piece[b.top[to as usize] as usize];
    }
    f[8] = on as f32 / 6.0;
    f[9] = opn as f32 / 6.0;
    f[10] = (qopp != NONE && to == qopp) as u8 as f32; // climbing onto the enemy queen
    f[11] = (qown != NONE && to == qown) as u8 as f32;
    MoveFeat { piece_tok, nb, f }
}

/// `thoraxrex policydump GAMES.jsonl OUT.txt [maxply] [who]` — one line per position:
/// T n | tokens (TF numbers each) | cells (q r per token, -99 for global) | M k | per move: piece nb*7 flags*MF | Y idx | W weight
/// `who`: comma list of player names whose moves are kept (default: everyone); weight = 1.
pub fn dump(args: &[String]) {
    use std::io::BufRead;
    let inp = args.get(0).expect("input");
    let out = args.get(1).expect("output");
    let maxply: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(200);
    let who: Vec<String> = args.get(3).map(|s| s.split(',').map(|x| x.to_string()).collect()).unwrap_or_default();
    let r = std::io::BufReader::new(std::fs::File::open(inp).expect("open input"));
    let mut f = std::io::BufWriter::new(std::fs::File::create(out).expect("create output"));
    let gt = GameType::parse("Base+MLP").unwrap();
    let (mut ngames, mut npos) = (0, 0);
    let mut legal = Vec::new();
    for line in r.lines() {
        let Ok(line) = line else { continue };
        let moves = crate::datagen::json_str_array(&line, "\"hist\"");
        let wname = crate::datagen::json_str(&line, "\"w\"").unwrap_or_default();
        let bname = crate::datagen::json_str(&line, "\"b\"").unwrap_or_default();
        let mut b = Board::new(gt);
        for (ply, mvs) in moves.iter().enumerate() {
            if ply >= maxply || b.result() != 0 {
                break;
            }
            let Ok(mv) = b.parse_move(mvs) else { break };
            let mover = if ply % 2 == 0 { &wname } else { &bname };
            let keep = who.is_empty() || who.iter().any(|w| w == mover);
            b.legal_moves(&mut legal);
            if keep && legal.len() > 1 && !mv.is_pass() {
                let e = encode(&b);
                let mut s = String::with_capacity(8192);
                s.push_str(&format!("T {}", e.tok.len()));
                for tk in &e.tok {
                    for v in tk {
                        push_num(&mut s, *v);
                    }
                }
                for c in &e.cell {
                    if *c == NONE {
                        s.push_str(" -99 -99");
                    } else {
                        s.push_str(&format!(" {} {}", c & 31, (c >> 5) & 31));
                    }
                }
                let lm: Vec<Move> = legal.iter().copied().filter(|m| !m.is_pass()).collect();
                s.push_str(&format!(" M {}", lm.len()));
                let mut y = usize::MAX;
                for (i, m) in lm.iter().enumerate() {
                    if *m == mv {
                        y = i;
                    }
                    let mf = move_features(&b, &e, *m);
                    s.push_str(&format!(" {}", mf.piece_tok));
                    for n in mf.nb {
                        s.push_str(&format!(" {}", n));
                    }
                    for v in mf.f {
                        push_num(&mut s, v);
                    }
                }
                if y != usize::MAX {
                    s.push_str(&format!(" Y {} P {}\n", y, ply));
                    f.write_all(s.as_bytes()).unwrap();
                    npos += 1;
                }
            }
            b.make(mv);
        }
        ngames += 1;
    }
    eprintln!("policydump: {} games, {} positions", ngames, npos);
}

fn push_num(s: &mut String, v: f32) {
    if v == 0.0 {
        s.push_str(" 0");
    } else if v == 1.0 {
        s.push_str(" 1");
    } else {
        s.push_str(&format!(" {:.4}", v));
    }
}

// ---------------------------------------------------------------- inference

pub struct Net {
    pub d: usize,
    pub heads: usize,
    pub layers: usize,
    pub ff: usize,
    w_in: Vec<f32>, // TF x d
    b_in: Vec<f32>,
    res_emb: Vec<f32>, // 8 x d
    lay: Vec<Layer>,
    lnf_g: Vec<f32>,
    lnf_b: Vec<f32>,
    m1: Vec<f32>, // (2d+MF) x d
    m1b: Vec<f32>,
    m2: Vec<f32>, // d
    m2b: f32,
}

struct Layer {
    ln1_g: Vec<f32>,
    ln1_b: Vec<f32>,
    wq: Vec<f32>,
    wk: Vec<f32>,
    wv: Vec<f32>,
    wo: Vec<f32>,
    bq: Vec<f32>,
    bk: Vec<f32>,
    bv: Vec<f32>,
    bo: Vec<f32>,
    dist_bias: Vec<f32>, // heads x DB
    ln2_g: Vec<f32>,
    ln2_b: Vec<f32>,
    f1: Vec<f32>, // d x ff
    f1b: Vec<f32>,
    f2: Vec<f32>, // ff x d
    f2b: Vec<f32>,
}

/// Weights file: header line `d heads layers ff`, then one tensor per line (`name n v1 v2 ...`) in a fixed order.
pub fn load(path: &str) -> Option<Net> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut lines = text.lines();
    let hdr: Vec<usize> = lines.next()?.split_whitespace().filter_map(|x| x.parse().ok()).collect();
    let (d, heads, layers, ff) = (hdr[0], hdr[1], hdr[2], hdr[3]);
    let mut next = |want: usize| -> Option<Vec<f32>> {
        let l = lines.next()?;
        let mut it = l.split_whitespace();
        let _name = it.next()?;
        let n: usize = it.next()?.parse().ok()?;
        let v: Vec<f32> = it.filter_map(|x| x.parse().ok()).collect();
        if n != want || v.len() != want {
            eprintln!("policy: bad tensor {} (want {}, got {} / {})", _name, want, n, v.len());
            return None;
        }
        Some(v)
    };
    let w_in = next(TF * d)?;
    let b_in = next(d)?;
    let res_emb = next(8 * d)?;
    let mut lay = Vec::new();
    for _ in 0..layers {
        lay.push(Layer {
            ln1_g: next(d)?,
            ln1_b: next(d)?,
            wq: next(d * d)?,
            bq: next(d)?,
            wk: next(d * d)?,
            bk: next(d)?,
            wv: next(d * d)?,
            bv: next(d)?,
            wo: next(d * d)?,
            bo: next(d)?,
            dist_bias: next(heads * DB)?,
            ln2_g: next(d)?,
            ln2_b: next(d)?,
            f1: next(d * ff)?,
            f1b: next(ff)?,
            f2: next(ff * d)?,
            f2b: next(d)?,
        });
    }
    let lnf_g = next(d)?;
    let lnf_b = next(d)?;
    let m1 = next((2 * d + MF) * d)?;
    let m1b = next(d)?;
    let m2 = next(d)?;
    let m2b = next(1)?[0];
    Some(Net { d, heads, layers, ff, w_in, b_in, res_emb, lay, lnf_g, lnf_b, m1, m1b, m2, m2b })
}

pub fn net() -> Option<&'static Net> {
    static N: std::sync::OnceLock<Option<Net>> = std::sync::OnceLock::new();
    N.get_or_init(|| {
        let p = std::env::var("TR_POLICY").ok().or_else(|| {
            std::env::current_exe().ok().and_then(|p| {
                let f = p.parent()?.join("thoraxrex-policy.txt");
                if f.exists() { Some(f.to_string_lossy().to_string()) } else { None }
            })
        })?;
        if p == "none" {
            return None;
        }
        let n = load(&p);
        if n.is_none() {
            eprintln!("policy: failed to load {}", p);
        }
        n
    })
    .as_ref()
}

fn layernorm(x: &[f32], g: &[f32], b: &[f32], out: &mut [f32]) {
    let n = x.len() as f32;
    let mean = x.iter().sum::<f32>() / n;
    let var = x.iter().map(|v| (v - mean) * (v - mean)).sum::<f32>() / n;
    let inv = 1.0 / (var + 1e-5).sqrt();
    for i in 0..x.len() {
        out[i] = (x[i] - mean) * inv * g[i] + b[i];
    }
}

/// y[j] = sum_i x[i] * w[i*cols + j] + bias[j]   (w stored input-major, as torch Linear.weight.T)
fn matvec(x: &[f32], w: &[f32], bias: &[f32], cols: usize, y: &mut [f32]) {
    y[..cols].copy_from_slice(&bias[..cols]);
    for (i, xi) in x.iter().enumerate() {
        if *xi == 0.0 {
            continue;
        }
        let row = &w[i * cols..(i + 1) * cols];
        for j in 0..cols {
            y[j] += xi * row[j];
        }
    }
}

fn gelu(x: f32) -> f32 {
    0.5 * x * (1.0 + (0.797_884_6 * (x + 0.044715 * x * x * x)).tanh())
}

impl Net {
    /// Logits for each move in `moves` (same order).
    pub fn logits(&self, b: &Board, moves: &[Move]) -> Vec<f32> {
        let e = encode(b);
        let mfs: Vec<Option<MoveFeat>> = moves.iter().map(|m| if m.is_pass() { None } else { Some(move_features(b, &e, *m)) }).collect();
        self.logits_from(&e.tok, &e.cell, &mfs)
    }

    pub fn logits_from(&self, tok: &[[f32; TF]], cells: &[u16], mfs: &[Option<MoveFeat>]) -> Vec<f32> {
        struct E<'a> { tok: &'a [[f32; TF]], cell: &'a [u16] }
        let e = E { tok, cell: cells };
        let n = e.tok.len();
        let d = self.d;
        let h = self.heads;
        let hd = d / h;
        let mut x = vec![0f32; n * d];
        for i in 0..n {
            matvec(&e.tok[i], &self.w_in, &self.b_in, d, &mut x[i * d..(i + 1) * d]);
        }
        // distance buckets
        let mut db = vec![0usize; n * n];
        for i in 0..n {
            for j in 0..n {
                db[i * n + j] = if e.cell[i] == NONE || e.cell[j] == NONE { 8 } else { (hexdist(e.cell[i], e.cell[j]) as usize).min(7) };
            }
        }
        let mut ln = vec![0f32; d];
        let mut q = vec![0f32; n * d];
        let mut k = vec![0f32; n * d];
        let mut v = vec![0f32; n * d];
        let mut att = vec![0f32; d];
        let mut tmp = vec![0f32; d];
        let mut hid = vec![0f32; self.ff];
        let scale = 1.0 / (hd as f32).sqrt();
        for l in &self.lay {
            for i in 0..n {
                layernorm(&x[i * d..(i + 1) * d], &l.ln1_g, &l.ln1_b, &mut ln);
                matvec(&ln, &l.wq, &l.bq, d, &mut q[i * d..(i + 1) * d]);
                matvec(&ln, &l.wk, &l.bk, d, &mut k[i * d..(i + 1) * d]);
                matvec(&ln, &l.wv, &l.bv, d, &mut v[i * d..(i + 1) * d]);
            }
            let mut newx = x.clone();
            let mut sc = vec![0f32; n];
            for i in 0..n {
                att.iter_mut().for_each(|a| *a = 0.0);
                for hh in 0..h {
                    let o = hh * hd;
                    let mut mx = f32::MIN;
                    for j in 0..n {
                        let mut s = 0.0;
                        for c in 0..hd {
                            s += q[i * d + o + c] * k[j * d + o + c];
                        }
                        s = s * scale + l.dist_bias[hh * DB + db[i * n + j]];
                        sc[j] = s;
                        mx = mx.max(s);
                    }
                    let mut z = 0.0;
                    for j in 0..n {
                        sc[j] = (sc[j] - mx).exp();
                        z += sc[j];
                    }
                    for j in 0..n {
                        let wgt = sc[j] / z;
                        for c in 0..hd {
                            att[o + c] += wgt * v[j * d + o + c];
                        }
                    }
                }
                matvec(&att, &l.wo, &l.bo, d, &mut tmp);
                for c in 0..d {
                    newx[i * d + c] += tmp[c];
                }
            }
            x = newx;
            for i in 0..n {
                layernorm(&x[i * d..(i + 1) * d], &l.ln2_g, &l.ln2_b, &mut ln);
                matvec(&ln, &l.f1, &l.f1b, self.ff, &mut hid);
                hid.iter_mut().for_each(|a| *a = gelu(*a));
                matvec(&hid, &l.f2, &l.f2b, d, &mut tmp);
                for c in 0..d {
                    x[i * d + c] += tmp[c];
                }
            }
        }
        let mut xo = vec![0f32; n * d];
        for i in 0..n {
            layernorm(&x[i * d..(i + 1) * d], &self.lnf_g, &self.lnf_b, &mut xo[i * d..(i + 1) * d]);
        }
        let mut out = Vec::with_capacity(mfs.len());
        let mut inp = vec![0f32; 2 * d + MF];
        let mut hm = vec![0f32; d];
        for mfo in mfs {
            let Some(mf) = mfo else {
                out.push(0.0);
                continue;
            };
            if mf.piece_tok >= 0 {
                let t = mf.piece_tok as usize;
                inp[..d].copy_from_slice(&xo[t * d..(t + 1) * d]);
            } else {
                let bug = (-mf.piece_tok - 1) as usize;
                inp[..d].copy_from_slice(&self.res_emb[bug * d..(bug + 1) * d]);
            }
            let mut cnt = 0.0;
            inp[d..2 * d].iter_mut().for_each(|a| *a = 0.0);
            for nbi in mf.nb {
                if nbi >= 0 {
                    let t = nbi as usize;
                    for c in 0..d {
                        inp[d + c] += xo[t * d + c];
                    }
                    cnt += 1.0;
                }
            }
            if cnt > 0.0 {
                for c in 0..d {
                    inp[d + c] /= cnt;
                }
            }
            inp[2 * d..].copy_from_slice(&mf.f);
            matvec(&inp, &self.m1, &self.m1b, d, &mut hm);
            let mut s = self.m2b;
            for c in 0..d {
                s += gelu(hm[c]) * self.m2[c];
            }
            out.push(s);
        }
        out
    }
}

/// `thoraxrex policycheck WEIGHTS DUMP N` — logits for the first N dump lines (to compare with the trainer's REF).
pub fn check(args: &[String]) {
    let net = load(&args[0]).expect("load weights");
    let text = std::fs::read_to_string(&args[1]).expect("dump");
    let n: usize = args.get(2).and_then(|x| x.parse().ok()).unwrap_or(1);
    let skip: usize = args.get(3).and_then(|x| x.parse().ok()).unwrap_or(0);
    for line in text.lines().skip(skip).take(n) {
        let p: Vec<&str> = line.split_whitespace().collect();
        let mut i = 1;
        let nt: usize = p[i].parse().unwrap();
        i += 1;
        let mut tok = vec![[0f32; TF]; nt];
        for t in 0..nt {
            for c in 0..TF {
                tok[t][c] = p[i].parse().unwrap();
                i += 1;
            }
        }
        let mut cells = vec![NONE; nt];
        for t in 0..nt {
            let q: i32 = p[i].parse().unwrap();
            let r: i32 = p[i + 1].parse().unwrap();
            i += 2;
            if q != -99 {
                cells[t] = cell(q, r);
            }
        }
        i += 1;
        let k: usize = p[i].parse().unwrap();
        i += 1;
        let mut mfs = Vec::new();
        for _ in 0..k {
            let piece_tok: i16 = p[i].parse::<f32>().unwrap() as i16;
            i += 1;
            let mut nb = [-1i16; NB];
            for s in 0..NB {
                nb[s] = p[i].parse::<f32>().unwrap() as i16;
                i += 1;
            }
            let mut f = [0f32; MF];
            for s in 0..MF {
                f[s] = p[i].parse().unwrap();
                i += 1;
            }
            mfs.push(Some(MoveFeat { piece_tok, nb, f }));
        }
        let z = net.logits_from(&tok, &cells, &mfs);
        println!("RUST {}", z.iter().take(8).map(|v| format!("{:.4}", v)).collect::<Vec<_>>().join(" "));
    }
}
