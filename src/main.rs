// GUI subsystem: no console window when launched from Explorer.
// In command-line mode we attach to the parent console explicitly.
#![windows_subsystem = "windows"]

use ycopy::{cli, gui, i18n::Translator, platform};

fn main() {
    let args = match cli::parse(std::env::args_os().skip(1)) {
        Ok(args) => args,
        Err(e) => {
            platform::attach_parent_console();
            let tr = Translator::detect(None);
            let msg = match e {
                cli::ParseError::UnknownOption(o) => tr.f("cli.err.unknown_option", &[&o]),
                cli::ParseError::MissingValue(o) => tr.f("cli.err.missing_value", &[&o]),
            };
            eprintln!("ycopy: {msg}");
            std::process::exit(2);
        }
    };
    let tr = Translator::detect(args.lang.as_deref());

    if args.help || args.version {
        platform::attach_parent_console();
        if args.version {
            println!("ycopy {}", env!("CARGO_PKG_VERSION"));
        } else {
            println!("{}", tr.t("cli.usage"));
        }
        return;
    }

    if args.files.is_empty() {
        // No file given: interactive mode (destinations may be pre-filled).
        gui::run(tr, args.dest1, args.dest2);
    } else {
        platform::attach_parent_console();
        std::process::exit(cli::run(args, &tr));
    }
}
