//! TODO

/// Command-line arguments
#[derive(Debug, ::clap::Parser)]
#[command(version, about=::clap::crate_description!(), long_about = ::clap::crate_description!())]
pub struct Arguments {
    /// The log level
    #[clap(flatten)]
    verbosity: ::clap_verbosity_flag::Verbosity<::clap_verbosity_flag::InfoLevel>,

    /// List of directories containing service definitions
    #[clap(required = true)]
    service_directories: Vec<::std::path::PathBuf>,
}

impl Arguments {
    /// TODO
    pub fn log_level_filter(&self) -> ::tracing_subscriber::filter::LevelFilter {
        self.verbosity.tracing_level_filter()
    }

    /// TODO
    pub fn services_directories(&self) -> &[::std::path::PathBuf] {
        &self.service_directories
    }

    #[cfg(test)]
    pub fn new_test(service_directories: Vec<::std::path::PathBuf>) -> Self {
        Self {
            verbosity: ::clap_verbosity_flag::Verbosity::new(2, 0),
            service_directories,
        }
    }
}

#[cfg(test)]
mod tests {
    use ::std::str::FromStr;

    use super::*;

    #[test]
    fn parse_log_level() {
        let arguments = <Arguments as ::clap::Parser>::try_parse_from(["sysinitd", "-vv", "/"])
            .expect("could not parse log-level (-vv) arguments");
        assert_eq!(
            arguments.log_level_filter(),
            ::tracing_subscriber::filter::LevelFilter::TRACE
        );
        let arguments = <Arguments as ::clap::Parser>::try_parse_from(["sysinitd", "-v", "/"])
            .expect("could not parse log-level (-v) arguments");
        assert_eq!(
            arguments.log_level_filter(),
            ::tracing_subscriber::filter::LevelFilter::DEBUG
        );
        let arguments = <Arguments as ::clap::Parser>::try_parse_from(["sysinitd", "-q", "/"])
            .expect("could not parse log-level (-q) arguments");
        assert_eq!(
            arguments.log_level_filter(),
            ::tracing_subscriber::filter::LevelFilter::WARN
        );
        let arguments = <Arguments as ::clap::Parser>::try_parse_from(["sysinitd", "-qq", "/"])
            .expect("could not parse log-level (-qq) arguments");
        assert_eq!(
            arguments.log_level_filter(),
            ::tracing_subscriber::filter::LevelFilter::ERROR
        );
        let arguments = <Arguments as ::clap::Parser>::try_parse_from(["sysinitd", "-qqq", "/"])
            .expect("could not parse log-level (-qqq) arguments");
        assert_eq!(
            arguments.log_level_filter(),
            ::tracing_subscriber::filter::LevelFilter::OFF
        );
    }

    #[test]
    fn test_services_directories() {
        let arguments = <Arguments as ::clap::Parser>::try_parse_from(["sysinitd", "/tmp"])
            .expect("could not parse single service-path argument");
        assert_eq!(
            arguments.log_level_filter(),
            ::tracing_subscriber::filter::LevelFilter::INFO
        );
        assert_eq!(
            arguments.services_directories(),
            &[::std::path::PathBuf::from_str("/tmp").unwrap()]
        );

        let arguments = <Arguments as ::clap::Parser>::try_parse_from(["sysinitd", "/tmp", "/usr"])
            .expect("could not parse two service-path arguments");
        assert_eq!(
            arguments.services_directories(),
            &[
                ::std::path::PathBuf::from_str("/tmp").unwrap(),
                ::std::path::PathBuf::from_str("/usr").unwrap()
            ]
        );
        let arguments =
            <Arguments as ::clap::Parser>::try_parse_from(["sysinitd", "/tmp", "/usr", "/etc"])
                .expect("could not parse three service-path arguments");
        assert_eq!(
            arguments.services_directories(),
            &[
                ::std::path::PathBuf::from_str("/tmp").unwrap(),
                ::std::path::PathBuf::from_str("/usr").unwrap(),
                ::std::path::PathBuf::from_str("/etc").unwrap()
            ]
        );
    }
}
