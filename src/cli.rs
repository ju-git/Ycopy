//! Command-line parsing and execution.

use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use crate::copier::{self, Event, Job};
use crate::format;
use crate::i18n::Translator;

#[derive(Debug, Default, PartialEq)]
pub struct Args {
    pub lang: Option<String>,
    pub dest1: Option<PathBuf>,
    pub dest2: Option<PathBuf>,
    pub files: Vec<PathBuf>,
    pub help: bool,
    pub version: bool,
}

#[derive(Debug, PartialEq)]
pub enum ParseError {
    UnknownOption(String),
    MissingValue(String),
}

/// Parses arguments (without the program name).
pub fn parse<I: IntoIterator<Item = OsString>>(args: I) -> Result<Args, ParseError> {
    let mut out = Args::default();
    let mut it = args.into_iter();
    let mut only_files = false;

    while let Some(arg) = it.next() {
        let text = arg.to_string_lossy().into_owned();
        if only_files || !text.starts_with('-') || text == "-" {
            out.files.push(PathBuf::from(arg));
            continue;
        }
        if text == "--" {
            only_files = true;
            continue;
        }
        // Support both `--opt value` and `--opt=value`.
        let (name, inline) = match text.split_once('=') {
            Some((n, v)) if n.starts_with("--") => (n.to_owned(), Some(OsString::from(v))),
            _ => (text.clone(), None),
        };
        let value = |it: &mut dyn Iterator<Item = OsString>| {
            inline.clone().or_else(|| it.next()).ok_or_else(|| ParseError::MissingValue(name.clone()))
        };
        match name.as_str() {
            "-h" | "--help" | "/?" => out.help = true,
            "-V" | "--version" => out.version = true,
            "--lang" => out.lang = Some(value(&mut it)?.to_string_lossy().into_owned()),
            "--dest1" => out.dest1 = Some(PathBuf::from(value(&mut it)?)),
            "--dest2" => out.dest2 = Some(PathBuf::from(value(&mut it)?)),
            _ => return Err(ParseError::UnknownOption(text)),
        }
    }
    Ok(out)
}

/// Runs the command-line copy. Returns the process exit code.
pub fn run(args: Args, tr: &Translator) -> i32 {
    let fail = |msg: String| {
        eprintln!("ycopy: {msg}");
        2
    };
    let (Some(dest1), Some(dest2)) = (args.dest1.clone(), args.dest2.clone()) else {
        return fail(tr.t("cli.err.missing_dest").to_owned());
    };
    if crate::same_folder(&dest1, &dest2) {
        return fail(tr.t("cli.err.same_dest").to_owned());
    }
    if let Some(bad) = args.files.iter().find(|p| !p.is_file()) {
        return fail(tr.f("cli.err.not_a_file", &[&bad.display().to_string()]));
    }

    let job = Job { files: args.files, dest1, dest2 };
    let cancel = AtomicBool::new(false);
    let sep = tr.decimal_separator().to_owned();
    let unit = tr.t("unit.rate").to_owned();
    let mut shown_width = 0;

    let result = copier::run(&job, &cancel, &mut |event| match event {
        Event::Progress(p) => {
            let eta = p.eta().map(format::duration).unwrap_or_else(|| "--".into());
            let rate = format!("{} {unit}", format::megabytes_per_second(p.rate(), &sep));
            let pct = (p.fraction() * 100.0) as u32;
            let n = (p.file_index + 1).min(p.total_files);
            let line = tr.f(
                "cli.progress",
                &[&n.to_string(), &p.total_files.to_string(), &pct.to_string(), &tr.f("status.progress", &[&eta, &rate])],
            );
            shown_width = shown_width.max(line.chars().count());
            eprint!("\r{line:<shown_width$}");
            let _ = std::io::stderr().flush();
        }
        Event::FileFailed { error, .. } => {
            eprint!("\r{:<shown_width$}\r", "");
            eprintln!("ycopy: {}", tr.copy_error(error));
        }
        Event::FileDone { .. } => {}
    });
    eprint!("\r{:<shown_width$}\r", "");

    match result {
        Err(e) => {
            eprintln!("ycopy: {}", tr.f("error.dest_folder", &[&e.to_string()]));
            1
        }
        Ok(s) => {
            let rate = format!("{} {unit}", format::megabytes_per_second(s.rate(), &sep));
            if s.failed.is_empty() {
                println!("{}", tr.f("status.done", &[&s.copied.to_string(), &format::duration(s.elapsed), &rate]));
                0
            } else {
                println!("{}", tr.f("status.partial", &[&s.copied.to_string(), &s.failed.len().to_string()]));
                1
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> Result<Args, ParseError> {
        parse(args.iter().map(OsString::from))
    }

    #[test]
    fn parses_options_and_files() {
        let a = p(&["--dest1", "a", "--dest2=b", "x.txt", "--lang", "fr", "y.txt"]).unwrap();
        assert_eq!(a.dest1, Some(PathBuf::from("a")));
        assert_eq!(a.dest2, Some(PathBuf::from("b")));
        assert_eq!(a.lang.as_deref(), Some("fr"));
        assert_eq!(a.files, vec![PathBuf::from("x.txt"), PathBuf::from("y.txt")]);
    }

    #[test]
    fn double_dash_ends_options() {
        let a = p(&["--", "--weird-name"]).unwrap();
        assert_eq!(a.files, vec![PathBuf::from("--weird-name")]);
    }

    #[test]
    fn reports_errors() {
        assert_eq!(p(&["--nope"]), Err(ParseError::UnknownOption("--nope".into())));
        assert_eq!(p(&["--dest1"]), Err(ParseError::MissingValue("--dest1".into())));
    }
}
