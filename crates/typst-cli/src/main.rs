mod args;
mod compile;
mod download;
mod fonts;
mod package;
mod query;
mod tracing;
#[cfg(feature = "self-update")]
mod update;
mod watch;
mod world;

use std::cell::Cell;
use std::env::args;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::process::ExitCode;

use anyhow::Result;
use aoko::no_std::algebraic::product::GErr;
use aoko::no_std::pipelines::tap::Tap;
use aoko::{val, var};
use args::{Command, CliArguments, CompileCommand, FontsCommand};
use clap::Parser;
use codespan_reporting::term::{self, termcolor};
use once_cell::sync::Lazy;
use termcolor::{ColorChoice, WriteColor};

thread_local! {
    /// The CLI's exit code.
    static EXIT: Cell<ExitCode> = Cell::new(ExitCode::SUCCESS);
}

/// The parsed commandline arguments.
static ARGS: Lazy<CliArguments> = Lazy::new(CliArguments::parse);

fn main() -> Result<()> {
    var!(args = args());
    val! {
        file_name = args.nth(1).ok_or(GErr("No file specified"))?;
        font = &args.next();
        font = font.as_deref().unwrap_or("HYKaiTiJ");
        tmp_file = "typst_inner_proc_intermediate_file";
        input = file_name.split(".").next().ok_or(GErr("File name error"))?;
        r#in = format!("
        #import \"@preview/cmarker:0.1.0\"
        #set text(font: \"{font}\")
        #cmarker.render(read(\"{file_name}\"))
    ")}
    if file_name == "fonts" {
        crate::fonts::fonts(&FontsCommand::default()).map_err(|e| GErr(e))?;
        return Ok(());
    }
    fs::write(tmp_file, r#in)?;
    let cc = CompileCommand::default()
        .tap_mut(|c| c.common.input = tmp_file.into())
        .tap_mut(|c| c.output = Some(input.into()))
        .tap_mut(|c| c.format = Some(args::OutputFormat::Pdf));
    crate::compile::compile(cc).map_err(|e| GErr(e))?;
    fs::remove_file(tmp_file)?;
    fs::rename(input, format!("{input}.pdf"))?;
    Ok(())
}

/// Entry point.
pub fn origin_main() -> ExitCode {
    let _guard = match crate::tracing::setup_tracing(&ARGS) {
        Ok(guard) => guard,
        Err(err) => {
            eprintln!("failed to initialize tracing ({err})");
            None
        }
    };

    let res = match &ARGS.command {
        Command::Compile(command) => crate::compile::compile(command.clone()),
        Command::Watch(command) => crate::watch::watch(command.clone()),
        Command::Query(command) => crate::query::query(command),
        Command::Fonts(command) => crate::fonts::fonts(command),
        Command::Update(command) => crate::update::update(command),
    };

    if let Err(msg) = res {
        set_failed();
        print_error(&msg).expect("failed to print error");
    }

    EXIT.with(|cell| cell.get())
}

/// Ensure a failure exit code.
fn set_failed() {
    EXIT.with(|cell| cell.set(ExitCode::FAILURE));
}

/// Print an application-level error (independent from a source file).
fn print_error(msg: &str) -> io::Result<()> {
    let mut w = color_stream();
    let styles = term::Styles::default();

    w.set_color(&styles.header_error)?;
    write!(w, "error")?;

    w.reset()?;
    writeln!(w, ": {msg}.")
}

/// Get stderr with color support if desirable.
fn color_stream() -> termcolor::StandardStream {
    termcolor::StandardStream::stderr(if std::io::stderr().is_terminal() {
        ColorChoice::Auto
    } else {
        ColorChoice::Never
    })
}

/// Used by `args.rs`.
fn typst_version() -> &'static str {
    env!("TYPST_VERSION")
}

#[cfg(not(feature = "self-update"))]
mod update {
    use crate::args::UpdateCommand;
    use typst::diag::{bail, StrResult};

    pub fn update(_: &UpdateCommand) -> StrResult<()> {
        bail!(
            "self-updating is not enabled for this executable, \
             please update with the package manager or mechanism \
             used for initial installation"
        )
    }
}
