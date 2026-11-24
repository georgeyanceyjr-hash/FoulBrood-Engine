mod board;
mod datagen;
mod book;
mod eval;
mod policy;
mod search;
mod uhp;

use std::io::{self, BufRead, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(|s| s.as_str()) == Some("datagen") {
        datagen::run(&args[2..]);
        return;
    }
    if args.get(1).map(|s| s.as_str()) == Some("policycheck") {
        policy::check(&args[2..]);
        return;
    }
    if args.get(1).map(|s| s.as_str()) == Some("policydump") {
        policy::dump(&args[2..]);
        return;
    }
    if args.get(1).map(|s| s.as_str()) == Some("relabel") {
        datagen::relabel(&args[2..]);
        return;
    }
    if args.get(1).map(|s| s.as_str()) == Some("neteval") {
        datagen::neteval(&args[2..]);
        return;
    }
    if args.get(1).map(|s| s.as_str()) == Some("dumpfeat") {
        datagen::dumpfeat(&args[2..]);
        return;
    }
    let stdin = io::stdin();
    let mut out = io::stdout();
    let mut engine = uhp::Engine::new();
    let emit = |out: &mut io::Stdout, lines: &[String]| {
        let mut lock = out.lock();
        for l in lines {
            let _ = writeln!(lock, "{}", l);
        }
        let _ = writeln!(lock, "ok");
        let _ = lock.flush();
    };
    emit(&mut out, &uhp::Engine::info());
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let t = line.trim();
        if t == "exit" || t == "quit" {
            break;
        }
        let r = engine.execute(t);
        let _ = io::stderr().flush();
        emit(&mut out, &r);
    }
}
