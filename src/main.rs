//! TODO

mod serialize;

use ::anyhow::Context as _;

/// TODO
#[derive(Debug, ::clap::Parser)]
#[command(version, about=::clap::crate_description!(), long_about = ::clap::crate_description!())]
struct Arguments {
    #[clap(flatten)]
    verbosity: ::clap_verbosity_flag::Verbosity<::clap_verbosity_flag::InfoLevel>,

    /// List of directories containing service definitions
    services_directories: Vec<::std::path::PathBuf>,
}

#[::tokio::main(flavor = "multi_thread")]
async fn main() {
    /// TODO
    async fn actual_main() -> ::anyhow::Result<()> {
        let arguments =
            <Arguments as ::clap::Parser>::try_parse().context("Could not parse arguments")?;

        ::tracing_subscriber::fmt()
            .with_max_level(arguments.verbosity)
            .with_target(false)
            .init();

        let mut services = tokio::task::JoinSet::new();

        for directory in &arguments.services_directories {
            let canonical_dir = directory.canonicalize().unwrap();
            if !canonical_dir.is_dir() {
                anyhow::bail!("{directory:?} is not a directory");
            }

            for dir_entry in std::fs::read_dir(canonical_dir).context(format!(
                "Could not loop over elements of provided directory {directory:?}"
            ))? {
                let dir_entry = match dir_entry {
                    Ok(dir_entry) => dir_entry,
                    Err(error) => ::anyhow::bail!("Could not read directory entry: {error}"),
                };

                let path = dir_entry.path();
                if !path.is_file() || path.extension().unwrap_or(std::ffi::OsStr::new("")) != "yaml"
                {
                    continue;
                }

                let file_content = std::fs::read(&path)
                    .context(format!("Could not read contents of file {path:?}"))?;

                let file_content = String::from_utf8(file_content).context(format!(
                    "Could not parse contents of file {path:?} into valid UTF-8"
                ))?;

                let service: serialize::Service = ::serde_yml::from_str(&file_content).unwrap();
                services.spawn(service.run());
            }
        }

        let mut final_result = Ok(());
        for result in services.join_all().await {
            if let Err(error) = result {
                if final_result.is_ok() {
                    final_result = Err(anyhow::Error::new(error));
                } else {
                    final_result = final_result.context(error);
                }
            }
        }
        if let Err(error) = final_result {
          final_result = Err(error.context("BIG FAILURE!"));
        }

        final_result
    }

    if let Err(error) = actual_main().await {
        ::tracing::error!("{error:?}");
        std::process::exit(1);
    }
}
