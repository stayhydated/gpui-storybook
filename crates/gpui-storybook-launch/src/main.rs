use gpui_storybook_launch::{LaunchCommand, LaunchOptions, SWAY_ENV_VAR};
use std::{ffi::OsString, process::ExitCode};

fn main() -> ExitCode {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_usage();
        return ExitCode::SUCCESS;
    }
    match parse_args(args)
        .and_then(|(command, options)| gpui_storybook_launch::run(&command, &options))
    {
        Ok(status) => ExitCode::from(status.code().unwrap_or(1).try_into().unwrap_or(1)),
        Err(error) => {
            eprintln!("gpui-storybook-launch: {error}");
            ExitCode::FAILURE
        },
    }
}

fn parse_args(
    args: impl IntoIterator<Item = OsString>,
) -> std::io::Result<(LaunchCommand, LaunchOptions)> {
    let mut args = args.into_iter();
    let mut options = LaunchOptions::default();
    let mut command = Vec::new();
    while let Some(arg) = args.next() {
        if !command.is_empty() {
            command.push(arg);
        } else if arg == "--" {
            command.extend(args);
            break;
        } else if arg == "--sway" {
            options.sway = Some(
                args.next()
                    .ok_or_else(|| std::io::Error::other("--sway requires an executable path"))?,
            );
        } else {
            return Err(std::io::Error::other(format!(
                "unexpected launcher argument `{}`; put the child command after `--`",
                arg.to_string_lossy()
            )));
        }
    }
    let mut command = command.into_iter();
    let program = command
        .next()
        .ok_or_else(|| std::io::Error::other("missing child command after `--`"))?;
    Ok((LaunchCommand::new(program, command), options))
}

fn print_usage() {
    println!(
        "Run a command in a private headless Sway session.\n\nUsage: gpui-storybook-launch [--sway PATH] -- COMMAND [ARGS...]\n\nEnvironment:\n  {SWAY_ENV_VAR}  Override the Sway executable"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_keeps_child_arguments_after_separator() {
        let (command, options) = parse_args([
            OsString::from("--sway"),
            OsString::from("/tmp/sway"),
            OsString::from("--"),
            OsString::from("cargo"),
            OsString::from("run"),
            OsString::from("--features"),
            OsString::from("mcp"),
        ])
        .expect("arguments should parse");
        assert_eq!(command.program(), "cargo");
        assert_eq!(command.args(), ["run", "--features", "mcp"]);
        assert_eq!(
            options.sway.as_deref(),
            Some(OsString::from("/tmp/sway").as_os_str())
        );
    }
}
