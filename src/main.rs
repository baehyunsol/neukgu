use neukgu::{
    Config,
    Context,
    Error,
    gui,
    init_working_dir,
    step,
    tui,
    validate_project_name,
};
use ragit_cli::{ArgCount, ArgParser, ArgType};
use ragit_fs::{create_dir, create_dir_all, exists, join};

const HELP_MESSAGE: &str = "
neukgu: an opinionated AI agent

Commands

neukgu new <project_name> [--instruction=<instruction>]
    creates a new project and initialize a neukgu directory

    If you don't give the instruction, you have to manually initialize the
    `neukgu-instruction.md` file.

neukgu init [--instruction=<instruction>]
    initializes a neukgu directory in the current directory

    If you don't give the instruction, you have to manually initialize the
    `neukgu-instruction.md` file.

neukgu headless [--working-dir=<path=.>] [--attach-fe]
    runs neukgu in the current directory

    It must already be initialized.
    You don't need this command unless you're building something on top of neukgu.
    This is mostly used by frontend, with --attach-fe flag.

neukgu tui [--no-backend]
    runs neukgu tui

    You can see neukgu working, but you can't interact with it.

neukgu gui
    runs neukgu gui

    If you're not sure, just run this command.
    GUI has all the features.
";

fn main() {
    let args = std::env::args().collect();

    match run(args) {
        Ok(()) => {},
        Err(e) => {
            match e {
                Error::FailedToAcquireWriteLock => {
                    eprintln!("It seems like another neukgu process is running in this directory.");
                },
                Error::IndexDirAlreadyExists => {
                    eprintln!("`.neukgu/` already exists.");
                },
                Error::IndexDirNotFound => {
                    eprintln!("`.neukgu/` not found.");
                },
                Error::CliError { message, span } => {
                    eprintln!("cli error: {message}{}",
                        if let Some(span) = span {
                            format!("\n\n{}", ragit_cli::underline_span(&span))
                        } else {
                            String::new()
                        },
                    );
                },
                e => {
                    eprintln!("{e:?}");
                },
            }

            std::process::exit(1);
        },
    }
}

fn run(args: Vec<String>) -> Result<(), Error> {
    match args.get(1).map(|s| s.as_str()) {
        Some("new") => {
            let parsed_args = ArgParser::new()
                .optional_arg_flag("--instruction", ArgType::String)
                .optional_flag(&["--mock-api"])
                .args(ArgType::String, ArgCount::Exact(1))
                .parse(&args, 2)?;

            let project_name = parsed_args.get_args_exact(1)?[0].clone();
            let instruction = parsed_args.arg_flags.get("--instruction").map(|s| s.to_string());
            let mock_api = parsed_args.get_flag(0).is_some();

            validate_project_name(&project_name)?;
            create_dir(&project_name)?;
            init_working_dir(instruction, &project_name, mock_api)?;
            Ok(())
        },
        Some("init") => {
            let parsed_args = ArgParser::new()
                .optional_arg_flag("--instruction", ArgType::String)
                .optional_flag(&["--mock-api"])
                .args(ArgType::String, ArgCount::None)
                .parse(&args, 2)?;

            let instruction = parsed_args.arg_flags.get("--instruction").map(|s| s.to_string());
            let mock_api = parsed_args.get_flag(0).is_some();

            init_working_dir(instruction, ".", mock_api)?;
            Ok(())
        },
        Some("headless") => {
            let parsed_args = ArgParser::new()
                .args(ArgType::String, ArgCount::None)
                .optional_arg_flag("--working-dir", ArgType::String)
                .optional_flag(&["--attach-fe"])
                .parse(&args, 2)?;

            let working_dir = parsed_args.arg_flags.get("--working-dir").map(|s| s.to_string()).unwrap_or(String::from("."));

            // If this flag is set, the backend loop runs only while the frontend is alive.
            let attach_fe = parsed_args.get_flag(0).is_some();

            if !exists(&join(&working_dir, ".neukgu/")?) {
                return Err(Error::IndexDirNotFound);
            }

            let config = Config::load(&working_dir)?;
            let mut context = Context::load(&config, &working_dir)?;

            if !exists(&config.sandbox_root) {
                create_dir_all(&config.sandbox_root)?;
            }

            if attach_fe {
                context.wait_for_fe()?;
            }

            let tokio_runtime = tokio::runtime::Runtime::new()?;

            tokio_runtime.block_on(async {
                loop {
                    if attach_fe && !context.is_fe_alive()? {
                        return Ok(());
                    }

                    if let Err(e) = step(&mut context, &config).await {
                        return Err(e);
                    }
                }
            })?;
            Ok(())
        },
        Some("tui") => {
            let parsed_args = ArgParser::new()
                .args(ArgType::String, ArgCount::None)
                .optional_flag(&["--no-backend"])
                .parse(&args, 2)?;

            let no_backend = parsed_args.get_flag(0).is_some();
            tui::run(no_backend, ".")
        },
        Some("gui") => {
            ArgParser::new()
                .args(ArgType::String, ArgCount::None)
                .parse(&args, 2)?;

            gui::run()
        },
        Some("help") => {
            println!("{HELP_MESSAGE}");
            Ok(())
        },
        _ => {
            println!("{HELP_MESSAGE}");
            std::process::exit(1)
        },
    }
}
