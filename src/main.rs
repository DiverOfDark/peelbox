use peelbox::cli::commands::{CliArgs, Commands};
use peelbox::cli::handlers::{handle_detect, handle_frontend, handle_health};
use peelbox::VERSION;

use clap::Parser;
use std::env;
use tracing::{debug, Level};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() {
    let args = CliArgs::parse();
    init_logging_from_args(&args);

    debug!("peelbox v{} starting", VERSION);
    debug!("Arguments: {:?}", args);

    let exit_code = match &args.command {
        Commands::Detect(detect_args) => handle_detect(detect_args, args.quiet, args.verbose).await,
        Commands::Health(health_args) => handle_health(health_args).await,
        Commands::Frontend(frontend_args) => handle_frontend(frontend_args).await,
    };

    std::process::exit(exit_code);
}

fn init_logging_from_args(args: &CliArgs) {
    use std::sync::Once;
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        let level = if let Some(level_str) = &args.log_level {
            parse_level(level_str)
        } else if args.verbose {
            Level::DEBUG
        } else if args.quiet {
            Level::ERROR
        } else {
            let level_str = env::var("PEELBOX_LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
            parse_level(&level_str)
        };

        let mut filter = EnvFilter::from_default_env();

        if env::var("RUST_LOG").is_err() {
            filter = filter
                .add_directive(format!("peelbox={}", level).parse().unwrap())
                .add_directive("h2=warn".parse().unwrap())
                .add_directive("hyper=warn".parse().unwrap())
                .add_directive("reqwest=warn".parse().unwrap());
        }

        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer().with_target(true).with_writer(std::io::stderr))
            .init();
    });
}

fn parse_level(level_str: &str) -> Level {
    match level_str.to_lowercase().as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => {
            eprintln!(
                "Invalid log level '{}', defaulting to INFO. Valid levels: trace, debug, info, warn, error",
                level_str
            );
            Level::INFO
        }
    }
}
