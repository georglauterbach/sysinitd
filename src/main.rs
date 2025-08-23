//! # `sysinitd`
//!
//! [`sysinitd`][sysinitd::github] is a simple supervisor daemon and
//! initialization process for Linux (containers): It starts and
//! supervises other processes. Its documentation is written entirely
//! via [`rustdoc`][rustdoc::documentation].
//!
//! ## How It Works
//!
//! 0. Startup Phase
//!    0. Early tracing framework initialization
//!    1. Parsing of arguments
//!    2. Execution of possible log level adjustments
//!    3. Execution of pre-run checks
//!    3. Parsing of service definitions
//! 1. Initialization Phase
//!    0. Execution of coherence checks for all service definitions
//!    1. Startup of processes
//!    2. Execution of post-start checks
//! 2. SupervisionPhase
//!    0. TODO
//! 3. Shutdown Phase
//!    0. TODO
//!
//! ## Technical Aspects
//!
//! ### Used Crates
//!
//! | Aspect              | Crate Name(s)                                             |
//! | :------------------ | :-------------------------------------------------------- |
//! | Argument Parsing    | [`clap`] + [`clap-verbosity-flag`], [`clap_autocomplete`] |
//! | Async Runtime       | [`tokio`]                                                 |
//! | Error Handling      | [`anyhow`], [`thiserror`]                                 |
//! | Service Definition  | [`serde`] and [`serde_yml`], [`humantime`], [`semver`]    |
//! | Tracing             | [`tracing`] and [`tracing-subscriber`]                    |
//!
//! [//]: # (Links)
//!
//! [sysinitd::github]: https://github.com/georglauterbach/sysinitd
//! [yaml::documentation]: https://en.wikipedia.org/wiki/YAML
//! [rustdoc::documentation]: https://doc.rust-lang.org/rustdoc/index.html

/// `sysinitd` starts here
///
/// [`::tokio`] builds a runtime and the [`run`] functions is called.
/// [`run`] is the "actual `main`" function that returns an
/// [`::anyhow::Result<()>`]. In case of an error, we display it and
/// abort.
#[::tokio::main(flavor = "multi_thread")]
async fn main() {
    if let Err(error) = run().await {
        ::tracing::error!("{error:?}");
        std::process::exit(1);
    }
}

/// Contains the actual functionality of `sysinitd`
async fn run() -> ::anyhow::Result<()> {
    let tracing_reload_handle = phases::startup::early_tracing_initialization();
    let arguments = phases::startup::parse_arguments()?;
    phases::startup::update_log_level(&arguments, &tracing_reload_handle)?;
    ::tracing::info!("Starting sysinitd v{}", env!("CARGO_PKG_VERSION"));
    phases::startup::pre_run_checks().await?;
    let process_definitions = phases::startup::parse_service_definitions(&arguments).await?;

    phases::initialization::coherence_checks(&process_definitions)?;
    phases::initialization::start_processes(&process_definitions);
    phases::initialization::post_start_checks();

    Ok(())
}

mod phases {
    //! Contains all phases

    pub mod startup {
        //! Contains all functionality of the early startup phase (0)

        use ::anyhow::Context as _;

        pub type TracingReloadHandle = ::tracing_subscriber::reload::Handle<
            ::tracing_subscriber::filter::LevelFilter,
            ::tracing_subscriber::Registry,
        >;

        /// The early log level to initialize `sysinitd` with
        const EARLY_LOG_LEVEL: ::tracing_subscriber::filter::LevelFilter =
            ::tracing_subscriber::filter::LevelFilter::TRACE;

        /// Performs an early initialization of the [`::tracing`] framework
        /// with [`::tracing_subscriber`]
        ///
        /// ## Why?
        ///
        /// An early initialization is required because we already require
        /// [`::tracing_subscriber`] to be initialized when argument parsing
        /// happens (which is the next function) to show possible errors.
        pub fn early_tracing_initialization() -> TracingReloadHandle {
            use tracing_subscriber::prelude::*;

            let (reload_layer, reload_handle) =
                ::tracing_subscriber::reload::Layer::new(EARLY_LOG_LEVEL);

            ::tracing_subscriber::registry()
                .with(reload_layer)
                .with(::tracing_subscriber::fmt::Layer::default().with_target(false))
                .init();

            ::tracing::trace!("Initialized tracing framework with log level '{EARLY_LOG_LEVEL}'");
            reload_handle
        }

        /// Parses [`sysinitd::Arguments`]
        pub fn parse_arguments() -> ::anyhow::Result<sysinitd::Arguments> {
            ::tracing::trace!("Parsing arguments");
            <sysinitd::Arguments as ::clap::Parser>::try_parse()
                .context("Could not parse arguments")
        }

        /// Updates the log level
        pub fn update_log_level(
            arguments: &sysinitd::Arguments,
            reload_handle: &TracingReloadHandle,
        ) -> anyhow::Result<()> {
            let new_level_filter = arguments.log_level_filter();

            if EARLY_LOG_LEVEL != new_level_filter {
                ::tracing::trace!("New log level is '{new_level_filter}'");
            }

            reload_handle
                // ! ATTENTION: We are NOT allowed to log messages in the following closure
                .modify(|filter| {
                    if *filter != new_level_filter {
                        *filter = new_level_filter;
                    }
                })
                .context("Could not update log level")
        }

        /// Performs pre-run checks and prints debug output
        pub async fn pre_run_checks() -> ::anyhow::Result<()> {
            ::tracing::info!("Executing pre-run checks");

            let kernel_version = String::from_utf8_lossy(
                &tokio::process::Command::new("uname")
                    .arg("-r")
                    .output()
                    .await
                    .context("Could not run or gather output of 'uname -r'")?
                    .stdout,
            )
            .trim()
            .to_string();
            tracing::debug!("Running on kernel version {kernel_version}");

            Ok(())
        }

        /// TODO
        pub async fn parse_service_definitions(
            arguments: &sysinitd::Arguments,
        ) -> anyhow::Result<std::collections::HashSet<sysinitd::Service>> {
            /// TODO
            async fn parse_service_directory(
                directory: std::path::PathBuf,
            ) -> ::anyhow::Result<Vec<sysinitd::Service>> {
                let canonical_dir = directory.canonicalize().unwrap();
                if !canonical_dir.is_dir() {
                    anyhow::bail!(
                        "Service directory '{}' is not a directory",
                        directory.display()
                    );
                }

                let mut services = Vec::with_capacity(4);

                for dir_entry in std::fs::read_dir(canonical_dir).context(format!(
                    "Could not loop over elements of provided directory {directory:?}"
                ))? {
                    let dir_entry = match dir_entry {
                        Ok(dir_entry) => dir_entry,
                        Err(error) => ::anyhow::bail!("Could not read directory entry: {error}"),
                    };

                    let path = dir_entry.path();
                    if !path.is_file()
                        || path
                            .extension()
                            .unwrap_or(std::ffi::OsStr::new(""))
                            .to_ascii_lowercase()
                            != "yaml"
                    {
                        continue;
                    }

                    ::tracing::debug!("Trying to read '{}'", path.display());

                    let file_content = std::fs::read(&path)
                        .context(format!("Could not read contents '{}'", path.display()))?;

                    let service: sysinitd::Service =
                        sysinitd::Service::serde_from_slice(&file_content, &path)?;

                    ::tracing::debug!("Parsed service '{}'", service.id());
                    services.push(service);
                }

                Ok(services)
            }

            ::tracing::info!("Parsing process definitions");

            let mut service_directory_parsers = ::tokio::task::JoinSet::new();

            for service_directory in arguments.services_directories() {
                service_directory_parsers.spawn(parse_service_directory(service_directory.clone()));
            }

            let mut services = std::collections::HashSet::with_capacity(8);
            let parsed_results = service_directory_parsers.join_all().await;
            for service_list in parsed_results {
                match service_list {
                    Ok(new_services) => {
                        for service in new_services {
                            let id = service.id().clone();
                            if !services.insert(service) {
                                ::anyhow::bail!("Service with ID '{id}' defined more than once");
                            }
                        }
                    }
                    Err(error) => ::anyhow::bail!(error),
                }
            }

            ::tracing::trace!("Parsed service definitions:\n{services:#?}\n");
            Ok(services)
        }

        #[cfg(test)]
        mod tests {
            use super::*;

            #[test]
            fn test_early_tracing_initialization() {
                early_tracing_initialization();
                assert_eq!(
                    ::tracing_subscriber::filter::LevelFilter::current(),
                    EARLY_LOG_LEVEL
                );
            }
        }
    }

    pub mod initialization {
        //! Contains all functionality of the initialization phase (1)

        /// TODO
        ///
        /// todo check cyclic dependencies
        pub fn coherence_checks(
            _process_definitions: &std::collections::HashSet<sysinitd::Service>,
        ) -> ::anyhow::Result<()> {
            ::tracing::info!("Running coherence checks on process definitions");

            // #[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
            // struct MyIndex(u32);

            // let mut g = ::petgraph::stable_graph::StableDiGraph::<MyIndex, ()>::new();
            // let y = g.add_node(MyIndex(1));
            // let z = g.add_node(MyIndex(1));
            // let mut _x = ::petgraph::acyclic::Acyclic::try_from(g).unwrap();
            // ::petgraph::acyclic::Acyclic::try_add_edge(&mut _x, y, z, ()).unwrap();
            // ::petgraph::acyclic::Acyclic::try_add_edge(&mut _x, z, y, ()).unwrap();
            // x.try_add_node(MyIndex(0));

            Ok(())
        }

        /// TODO
        pub fn start_processes(
            _process_definitions: &std::collections::HashSet<sysinitd::Service>,
        ) {
            ::tracing::info!("Starting processes");
        }

        /// TODO
        pub fn post_start_checks() {}
    }
}
