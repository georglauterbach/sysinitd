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
//!    2. Execution of possible log level updates
//!    3. Execution of environment checks
//!    4. Parsing of service definitions
//!    5. Execution of checks on service definitions
//! 1. Initialization Phase
//!    0. Startup of processes
//!    1. Execution of post-start checks
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

use ::anyhow::Context;

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
    let tracing_reload_handle = phases::startup::initialize_tracing_early();
    let arguments = phases::startup::parse_arguments()?;
    phases::startup::update_log_level(&arguments, &tracing_reload_handle)?;
    ::tracing::info!("Starting sysinitd v{}", env!("CARGO_PKG_VERSION"));
    phases::startup::execute_environment_checks().await?;
    let process_definitions = phases::startup::parse_service_definitions(&arguments).await?;
    phases::startup::check_service_definitions(&process_definitions)
        .context("Service definition checks failed")?;

    phases::initialization::start_services(&process_definitions);
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
        pub fn initialize_tracing_early() -> TracingReloadHandle {
            use tracing_subscriber::prelude::*;

            let (reload_layer, reload_handle) =
                ::tracing_subscriber::reload::Layer::new(EARLY_LOG_LEVEL);

            ::tracing_subscriber::registry()
                .with(reload_layer)
                .with(::tracing_subscriber::fmt::Layer::default().with_target(false))
                .init();

            ::tracing::trace!(
                "Early initialization of tracing framework with log level '{EARLY_LOG_LEVEL}' completed"
            );
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
                ::tracing::trace!("Changing log level to '{new_level_filter}'");
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

        /// Performs environment checks and prints debug output
        pub async fn execute_environment_checks() -> ::anyhow::Result<()> {
            ::tracing::info!("Executing environment checks");

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
        ) -> anyhow::Result<std::collections::HashMap<String, sysinitd::Service>> {
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
                    let path_extension = path
                        .extension()
                        .unwrap_or(std::ffi::OsStr::new(""))
                        .to_ascii_lowercase();

                    if path_extension == "yml" {
                        ::tracing::warn!(
                            "Please use the file extension '.yaml' and not '.yml' with '{}'",
                            path.display()
                        )
                    }

                    if !path.is_file() || (path_extension != "yaml" && path_extension != "yml") {
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

            let mut services = std::collections::HashMap::with_capacity(8);
            let parsed_results = service_directory_parsers.join_all().await;
            for service_list in parsed_results {
                match service_list {
                    Ok(new_services) => {
                        for service in new_services {
                            let id = service.id().clone();
                            if services.insert(service.id().clone(), service).is_some() {
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

        /// TODO
        #[derive(Debug, PartialEq)]
        enum DependencyError {
            /// TODO
            Cycle(String),
            /// TODO
            NonExistent(String, String),
            /// TODO
            Other(String),
        }

        impl std::fmt::Display for DependencyError {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    Self::Cycle(cycle) => {
                        write!(formatter, "Your dependencies form a cycle: {cycle}")
                    }
                    Self::NonExistent(service, dependency) => write!(
                        formatter,
                        "Dependency '{dependency}' of service '{service}' does not exist"
                    ),
                    Self::Other(message) => write!(formatter, "{message}"),
                }
            }
        }

        impl std::error::Error for DependencyError {}

        /// TODO
        fn check_cyclic_dependencies<'a>(
            service_definitions: &'a std::collections::HashMap<String, sysinitd::Service>,
            services_visited: &mut Vec<&'a sysinitd::Service>,
            services_checked: &mut std::collections::HashSet<&'a sysinitd::Service>,
        ) -> Result<(), DependencyError> {
            let service_current = *services_visited.last().ok_or(DependencyError::Other("bug: 'phases::initialization::check_cyclic_dependencies()' received an empty 'services_visited'".to_string()))?;

            if services_checked.contains(service_current) {
                return Ok(());
            }

            let service_current_id = service_current.id();

            if let sysinitd::service::Start {
                dependencies: Some(dependencies_of_new_service),
                ..
            } = service_current.start()
            {
                if dependencies_of_new_service.contains(service_current_id) {
                    // we detected a loop to self - return with `Err`
                    return Err(DependencyError::Cycle(format!(
                        "<-> {service_current_id} ('{service_current_id}' lists itself as a dependency)",
                    )));
                }

                for dependency_of_new_service_name in dependencies_of_new_service {
                    let dependency_of_new_service = service_definitions
                        .get(dependency_of_new_service_name)
                        .ok_or(DependencyError::NonExistent(
                            service_current_id.clone(),
                            dependency_of_new_service_name.clone(),
                        ))?;

                    if services_visited.contains(&dependency_of_new_service) {
                        // we detected a loop - return with `Err`
                        return Err(DependencyError::Cycle(format!(
                            "-> {dependency_of_new_service_name}"
                        )));
                    } else {
                        services_visited.push(dependency_of_new_service);
                        check_cyclic_dependencies(
                            service_definitions,
                            services_visited,
                            services_checked,
                        )
                        .map_err(|error| match error {
                            DependencyError::Cycle(message) => DependencyError::Cycle(format!(
                                "-> {dependency_of_new_service_name} {message}"
                            )),
                            _ => error,
                        })?;
                        services_checked.insert(dependency_of_new_service);
                    }
                }
            }

            for services_visited in services_visited {
                services_checked.insert(services_visited);
            }

            Ok(())
        }

        /// TODO
        pub fn check_service_definitions(
            service_definitions: &std::collections::HashMap<String, sysinitd::Service>,
        ) -> ::anyhow::Result<()> {
            ::tracing::info!("Executing service definition checks");

            // an efficient measure to prevent infinite recursion: we list the nodes we already checked
            // and do not check them again; this is also a nice optimization
            let mut already_checked_for_cycles = std::collections::HashSet::with_capacity(8);

            for (service_current_name, service_current) in service_definitions {
                check_cyclic_dependencies(
                    service_definitions,
                    &mut vec![service_current],
                    &mut already_checked_for_cycles,
                )
                .map_err(|error| {
                    if let DependencyError::Cycle(message) = error {
                        DependencyError::Cycle(format!("{service_current_name} {message}"))
                    } else {
                        error
                    }
                })
                .context("Service dependencies are invalid")?;
            }

            Ok(())
        }

        #[cfg(test)]
        mod tests {
            use ::std::str::FromStr;

            use super::*;

            #[test]
            fn test_initialize_tracing_early() {
                initialize_tracing_early();
                assert_eq!(
                    ::tracing_subscriber::filter::LevelFilter::current(),
                    EARLY_LOG_LEVEL
                );
            }

            /// TODO
            async fn create_service_definitions(
                testdata_dir: impl AsRef<str>,
            ) -> ::anyhow::Result<std::collections::HashMap<String, sysinitd::Service>>
            {
                let mut path_to_service_definitions =
                    std::path::PathBuf::from_str(env!("CARGO_MANIFEST_DIR"))
                        .expect("Could not build path to workspace directory");
                path_to_service_definitions = path_to_service_definitions.join("assets/tests");
                path_to_service_definitions =
                    path_to_service_definitions.join(testdata_dir.as_ref());

                let arguments = <sysinitd::Arguments as ::clap::Parser>::parse_from([
                    "sysinitd",
                    "-vv",
                    path_to_service_definitions.to_str().expect(
                        "Could not construct valid string from path to service definitions",
                    ),
                ]);

                parse_service_definitions(&arguments).await
            }

            #[::tokio::test]
            async fn dependencies_circle_big() {
                let service_definitions =
                    create_service_definitions("services/dependencies/circle_big")
                        .await
                        .expect("Could not parse service defintions");
                let result = check_service_definitions(&service_definitions);
                assert!(result.is_err());
                let error = result.unwrap_err();
                let error = error
                    .downcast::<DependencyError>()
                    .expect("The error must be a 'DependencyError'");
                assert!(matches!(error, DependencyError::Cycle(..)));
            }

            #[::tokio::test]
            async fn dependencies_circle_small() {
                let service_definitions =
                    create_service_definitions("services/dependencies/circle_small")
                        .await
                        .expect("Could not parse service defintions");
                let result = check_service_definitions(&service_definitions);
                assert!(result.is_err());
                let error = result.unwrap_err();
                let error = error
                    .downcast::<DependencyError>()
                    .expect("The error must be a 'DependencyError'");
                assert!(matches!(error, DependencyError::Cycle(..)));
            }

            #[::tokio::test]
            async fn dependencies_circle_self() {
                let service_definitions =
                    create_service_definitions("services/dependencies/circle_self")
                        .await
                        .expect("Could not parse service defintions");
                let result = check_service_definitions(&service_definitions);
                assert!(result.is_err());
                let error = result.unwrap_err();
                let error = error
                    .downcast::<DependencyError>()
                    .expect("The error must be a 'DependencyError'");
                assert_eq!(
                    error,
                    DependencyError::Cycle(String::from(
                        "service-a <-> service-a ('service-a' lists itself as a dependency)"
                    ))
                );
            }

            #[::tokio::test]
            async fn dependencies_nonexistent() {
                let service_definitions =
                    create_service_definitions("services/dependencies/nonexistent")
                        .await
                        .expect("Could not parse service defintions");
                let result = check_service_definitions(&service_definitions);
                assert!(result.is_err());
                let error = result.unwrap_err();
                let error = error
                    .downcast::<DependencyError>()
                    .expect("The error must be a 'DependencyError'");
                assert_eq!(
                    error,
                    DependencyError::NonExistent(
                        String::from("service-a"),
                        String::from("service-b")
                    )
                );
            }

            #[::tokio::test]
            async fn services_non_unique_id() {
                let service_definitions =
                    create_service_definitions("services/non_unique").await;
                assert!(service_definitions.is_err());
                let error = service_definitions.unwrap_err();
                assert_eq!(&error.to_string(), "Service with ID 'service-a' defined more than once");
            }
        }
    }

    pub mod initialization {
        //! Contains all functionality of the initialization phase (1)

        /// TODO
        pub fn start_services(
            _service_definitions: &std::collections::HashMap<String, sysinitd::Service>,
        ) {
            ::tracing::info!("Starting processes");
        }

        /// TODO
        pub fn post_start_checks() {}
    }
}
