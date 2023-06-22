use std::sync::Arc;

use anyhow::Result;
use clap::StructOpt;
use duo_subscriber::DuoLayer;
use parking_lot::RwLock;
use tracing::Level;
use tracing_subscriber::{
    filter::{self, Targets},
    fmt,
    layer::SubscriberExt,
    util::SubscriberInitExt,
    Layer,
};

mod aggregator;
mod arrow;
mod grpc;
mod models;
mod partition;
mod query;
mod utils;
mod warehouse;
mod web;

pub use aggregator::Aggregator;
pub use grpc::spawn_server as spawn_grpc_server;
pub use models::{Log, Process, Span, TraceExt};
pub use warehouse::Warehouse;
pub use web::run_web_server;

// ASCII Art generated from https://patorjk.com/software/taag/#p=display&h=0&v=0&f=ANSI%20Regular&t=Duo
static DUO_BANNER: &str = r"
                                  
██████  ██    ██  ██████  
██   ██ ██    ██ ██    ██ 
██   ██ ██    ██ ██    ██ 
██   ██ ██    ██ ██    ██ 
██████   ██████   ██████  
                                  
";

#[derive(Debug, clap::Parser)]
#[clap(name = "duo")]
#[clap(about = "Observability duo: Logging and Tracing.", long_about = None)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Debug, clap::Subcommand)]
enum Commands {
    /// Start the duo server.
    Start {
        /// The web server listening port.
        #[clap(short, default_value_t = 3000)]
        web_port: u16,
        /// The gRPC server listening port.
        #[clap(short, default_value_t = 6000)]
        grpc_port: u16,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("{}", DUO_BANNER);
    let warehouse = Arc::new(RwLock::new(Warehouse::load(".")?));

    match Cli::parse().command {
        Commands::Start {
            web_port,
            grpc_port,
        } => {
            spawn_grpc_server(Arc::clone(&warehouse), grpc_port);

            let duo_layer = DuoLayer::new(
                "duo",
                format!("grpc://127.0.0.1:{}", grpc_port).parse().unwrap(),
            )
            .await;
            tracing_subscriber::registry()
                .with(fmt::layer())
                .with(duo_layer.with_filter(filter::filter_fn(|metadata| {
                    // Ignore "duo_internal" event to avoid recursively report event to duo-server
                    metadata.target() != "duo_internal"
                })))
                .with(Targets::new().with_target("duo", Level::DEBUG))
                .init();

            run_web_server(warehouse, web_port).await?;
        }
    }
    Ok(())
}
