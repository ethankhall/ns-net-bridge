mod error;

use clap::Clap;
use log::{info, debug, error};

use tokio::runtime::Builder;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::io;
use tokio::net::{TcpListener, TcpStream};
use futures::{FutureExt, future::try_join};

use error::{CliErrors};

#[derive(Clap, Debug)]
struct LoggingOpts {
    /// A level of verbosity, and can be used multiple times
    #[clap(short, long, parse(from_occurrences), group = "logging")]
    verbose: u64,

    /// Enable all logging
    #[clap(short, long, group = "logging")]
    debug: bool,

    /// Disable everything but error logging
    #[clap(short, long, group = "logging")]
    error: bool,
}

#[derive(Clap, Debug)]
struct Opts {
    #[clap(subcommand)]
    subcmd: SubCommand,
    #[clap(flatten)]
    logging_opts: LoggingOpts,
}

#[derive(Clap, Debug)]
enum SubCommand {
    Forward(ForwardArgs),
    Fetch(FetchArgs)
}

#[derive(Clap, Debug)]
struct FetchArgs {
    /// File to the network namespace to attach to.
    #[clap(long)]
    network_namespace_file: String,
    /// Address in the namespace to execute
    #[clap(short, long)]
    address: String,
    #[clap(flatten)]
    logging_opts: LoggingOpts,
}

#[derive(Clap, Debug)]
struct ForwardArgs {
    /// File to the network namespace to attach to.
    #[clap(long)]
    network_namespace_file: String,
    /// Address in the namespace to forward to
    #[clap(long)]
    namespace_address: String,
    /// Address on the host to listen on
    #[clap(long)]
    host_address: String,
    #[clap(flatten)]
    logging_opts: LoggingOpts,
}

fn main() {
    let opt = Opts::parse();

    let result = match opt.subcmd {
        SubCommand::Forward(fwd) => exec_forward(fwd),
        SubCommand::Fetch(fetch) => exec_fetch(fetch)
    };

    if let Err(e) = result {
        error!("Error: {}", e);
        std::process::exit(e.get_error_number().into());
    }
}

fn init_logger(logging_opts: &LoggingOpts) {
    let mut logger = loggerv::Logger::new();
    if logging_opts.debug {
        logger = logger
            .verbosity(10)
            .line_numbers(true);
    } else if logging_opts.error {
        logger = logger.verbosity(0);
    } else {
        logger = logger
            .verbosity(logging_opts.verbose)
            .base_level(log::Level::Info)
            .add_module_path_filter(module_path!());
    }

    logger.init().unwrap();
}

fn exec_fetch(args: FetchArgs) -> Result<(), CliErrors> {
    init_logger(&args.logging_opts);

    let mut runtime = Builder::new()
        .enable_all()
        .threaded_scheduler()
        .core_threads(4)
        .build()
        .unwrap();
    
    runtime.block_on(async {
        debug!("Network Namespace: {:?}", args.network_namespace_file);
        enter_network_namespace(args.network_namespace_file)?;

        let results = reqwest::get(&args.address).await?;
        info!("Response: {:?}", results);
       
        
        Ok(())
    })
}

// This code works in strange ways. This is mostly becuase of network namespaces are bound to
// thread. And a thread can exist in exactly one network namespace at a time. So for the
// functionality to work correctly, you need to make a second thread that can be executed 
// inside the target namespace. Then you must copy data back and forth between the two
// threads to send data.
fn exec_forward(args: ForwardArgs) -> Result<(), CliErrors> {
    init_logger(&args.logging_opts);

    let mut host_runtime = Builder::new()
        .basic_scheduler()
        .enable_all()
        .build()
        .unwrap();

    let network_namespace_file = args.network_namespace_file.clone();

    let container_runtime = Builder::new()
        .threaded_scheduler()
        .on_thread_start(move || {
            enter_network_namespace(network_namespace_file.clone()).unwrap();
        })
        .enable_all()
        .build()
        .unwrap();

    let proxy_addr = args.namespace_address.clone();        

    host_runtime.block_on( async {
        let mut listener = TcpListener::bind(&args.host_address).await?;

        info!("Listening on {}", args.host_address);

        while let Ok((inbound, incoming_addr)) = listener.accept().await {

            info!("New connection from {}", incoming_addr);

            let (to_container, from_host) = inbound.into_split();

            let transfer = transfer(to_container, from_host, proxy_addr.clone()).map(|r| {
                if let Err(e) = r {
                    println!("Failed to transfer; error={}", e);
                }
            });
    
            container_runtime.spawn(transfer);
        }

        Ok(())
    })
}

#[cfg(target_os = "macos")]
fn enter_network_namespace(_namespace_path: String) -> Result<(), CliErrors> {
    Ok(())
}

#[cfg(target_os = "linux")]
fn enter_network_namespace(namespace_path: String) -> Result<(), CliErrors> {
    use std::os::unix::io::IntoRawFd;
    use std::fs::File;
    use nix::sched::{setns, CloneFlags};

    let namespace_file = File::open(namespace_path)?;
    let namespace_file = namespace_file.into_raw_fd();
    debug!("Entering namespace");
    setns(namespace_file, CloneFlags::CLONE_NEWNET)?;
    debug!("Entered namespace");

    Ok(())
}

async fn transfer(mut to_container: OwnedReadHalf, mut from_host: OwnedWriteHalf, proxy_addr: String) -> Result<(), Box<dyn std::error::Error>> {
    let outbound = TcpStream::connect(proxy_addr).await?;

    let (mut to_host, mut from_container) = outbound.into_split();
    let client_to_server = io::copy(&mut to_container, &mut from_container);
    let server_to_client = io::copy(&mut to_host, &mut from_host);

    try_join(client_to_server, server_to_client).await?;

    Ok(())
}