use neukgu::{
    Config,
    Context,
    Error,
    gui,
    init_working_dir,
    step,
    tui,
};
use ragit_cli::{ArgCount, ArgParser, ArgType};
use ragit_fs::{create_dir, create_dir_all, exists, set_current_dir};

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
                .args(ArgType::String, ArgCount::Exact(1))
                .parse(&args, 2)?;

            let project_name = parsed_args.get_args_exact(1)?[0].clone();
            let instruction = parsed_args.arg_flags.get("--instruction").map(|s| s.to_string());

            create_dir(&project_name)?;
            set_current_dir(&project_name)?;
            init_working_dir(instruction)?;
            Ok(())
        },
        Some("init") => {
            let parsed_args = ArgParser::new()
                .optional_arg_flag("--instruction", ArgType::String)
                .args(ArgType::String, ArgCount::None)
                .parse(&args, 2)?;

            let instruction = parsed_args.arg_flags.get("--instruction").map(|s| s.to_string());

            init_working_dir(instruction)?;
            Ok(())
        },
        Some("headless") => {
            let parsed_args = ArgParser::new()
                .args(ArgType::String, ArgCount::None)
                .optional_flag(&["--attach-fe"])
                .parse(&args, 2)?;

            // If this flag is set, the backend loop runs only while the frontend is alive.
            let attach_fe = parsed_args.get_flag(0).is_some();

            if !exists(".neukgu/") {
                return Err(Error::IndexDirNotFound);
            }

            let config = Config::load()?;
            let mut context = Context::load(&config)?;

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
            tui::run(no_backend)
        },
        Some("gui") => {
            let parsed_args = ArgParser::new()
                .args(ArgType::String, ArgCount::None)
                .optional_flag(&["--no-backend"])
                .parse(&args, 2)?;

            let no_backend = parsed_args.get_flag(0).is_some();
            gui::run(no_backend)
        },
        Some("help") => todo!(),
        _ => todo!(),
    }
}
