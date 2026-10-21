//! Command line entry: open a window, or time the engine without one.

use std::path::PathBuf;
use std::process::ExitCode;

use citerack_store::Session;

const HELP: &str = "\
citerack - a reference manager for large BibTeX libraries

Usage:
    citerack [FILE.bib]            open a library in a window
    citerack --open-time FILE.bib  parse and index, print the timings, exit
    citerack --search QUERY FILE   print the first rows a query matches, exit
    citerack --explain QUERY FILE  print what the index holds per term, exit
    citerack --index-report FILE   print where the index spends its memory, exit
    citerack --version             print the version
    citerack --help                print this text
";

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("--help" | "-h") => {
            println!("{HELP}");
            ExitCode::SUCCESS
        }
        Some("--version" | "-V") => {
            println!("citerack {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some("--search") => match (args.next(), args.next()) {
            (Some(query), Some(path)) => search(PathBuf::from(path), &query),
            _ => usage_error("--search needs a query and a file"),
        },
        Some("--explain") => match (args.next(), args.next()) {
            (Some(query), Some(path)) => explain(PathBuf::from(path), &query),
            _ => usage_error("--explain needs a query and a file"),
        },
        Some("--index-report") => match args.next() {
            Some(path) => index_report(PathBuf::from(path)),
            None => usage_error("--index-report needs a file"),
        },
        Some("--open-time") => match args.next() {
            Some(path) => open_time(PathBuf::from(path)),
            None => usage_error("--open-time needs a file"),
        },
        Some(flag) if flag.starts_with('-') => usage_error(&format!("unknown option {flag}")),
        Some(path) => gui(Some(PathBuf::from(path))),
        None => gui(None),
    }
}

fn usage_error(message: &str) -> ExitCode {
    eprintln!("{message}\n\n{HELP}");
    ExitCode::from(2)
}

/// Prints how many postings each column holds, which is where the memory goes
/// and which column another saving would have to come from.
fn index_report(path: PathBuf) -> ExitCode {
    match Session::open(&path) {
        Ok(session) => {
            session.report();
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("{}: {err}", path.display());
            ExitCode::FAILURE
        }
    }
}

/// Prints how long opening took, which is the figure a bug report should quote.
fn open_time(path: PathBuf) -> ExitCode {
    let bytes = match std::fs::metadata(&path) {
        Ok(meta) => meta.len(),
        Err(err) => {
            eprintln!("{}: {err}", path.display());
            return ExitCode::FAILURE;
        }
    };
    let started = std::time::Instant::now();
    match Session::open(&path) {
        Ok(session) => {
            let total = started.elapsed().as_secs_f64();
            println!("file        {}", path.display());
            println!("size        {:.2} MiB", bytes as f64 / (1024.0 * 1024.0));
            println!("entries     {}", session.len());
            println!("open_total  {total:.3} s");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("{}: {err}", path.display());
            ExitCode::FAILURE
        }
    }
}

/// Prints what a query would show, which is how the engine is tested without a
/// window and how a bug report shows what the reporter typed.
fn search(path: PathBuf, query: &str) -> ExitCode {
    let mut session = match Session::open(&path) {
        Ok(session) => session,
        Err(err) => {
            eprintln!("{}: {err}", path.display());
            return ExitCode::FAILURE;
        }
    };
    let started = std::time::Instant::now();
    let result = session.search(query);
    let elapsed = started.elapsed().as_secs_f64();
    println!("query       {query:?}");
    println!("matched     {}", result.total);
    println!("search      {:.3} ms", elapsed * 1000.0);
    for row in result.rows.iter().take(10) {
        let Some(entry) = session.library().entries().get(*row) else {
            continue;
        };
        println!("  {:<24} {}", entry.key, entry.field("title").unwrap_or(""));
    }
    ExitCode::SUCCESS
}

/// Prints what the index holds for each term of a query, which is how a slow or
/// empty result is explained without a window.
fn explain(path: PathBuf, query: &str) -> ExitCode {
    match Session::open(&path) {
        Ok(session) => {
            session.explain(query);
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("{}: {err}", path.display());
            ExitCode::FAILURE
        }
    }
}

fn gui(path: Option<PathBuf>) -> ExitCode {
    match citerack::run(path) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("cannot open a window: {err}");
            ExitCode::FAILURE
        }
    }
}
