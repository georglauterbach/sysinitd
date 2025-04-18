//! TODO

use ::std::fmt::write;

use ::anyhow::Context;
use ::serde::Deserialize;

/// TODO
#[derive(Debug, ::clap::Parser)]
#[command(version, about=::clap::crate_description!(), long_about = ::clap::crate_description!())]
struct Arguments {
    #[clap(flatten)]
    verbosity: ::clap_verbosity_flag::Verbosity<::clap_verbosity_flag::InfoLevel>,

    /// List of directories containing service definitions
    services_directories: Vec<::std::path::PathBuf>,
}

#[derive(Debug, ::serde::Deserialize, ::serde::Serialize)]
struct Meta {
    version: String,
}

fn deserialize_option_humantime<'de, D>(deserializer: D) -> Result<Option<std::time::Duration>, D::Error>
where
    D: ::serde::Deserializer<'de>,
{
    let deserialized_string = String::deserialize(deserializer)?;
    match ::humantime::parse_duration(&deserialized_string) {
      Ok(duration) => Ok(Some(duration)),
      Err(error) => Err(::serde::de::Error::custom(error))
    }
}

#[derive(Debug, ::serde::Deserialize, ::serde::Serialize)]
struct Start {
    command: String,
    arguments: Vec<String>,
    user: String,
    after: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "deserialize_option_humantime")]
    delay: Option<std::time::Duration>,
}

#[derive(Debug, ::serde::Deserialize, ::serde::Serialize)]
struct Service {
    meta: Meta,
    id: String,
    start: Start,
}

impl std::fmt::Display for Service {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      write!(f, "Service: {{
{}}}", ::serde_yml::to_string(self).unwrap())
  }
}

#[::tokio::main(flavor = "multi_thread")]
async fn main() {
    /// TODO
    fn actual_main() -> ::anyhow::Result<()> {
        let arguments =
            <Arguments as ::clap::Parser>::try_parse().context("Could not parse arguments")?;

        ::tracing_subscriber::fmt()
            .with_max_level(arguments.verbosity)
            .with_target(false)
            .init();

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

                let service: Service = ::serde_yml::from_str(&file_content).unwrap();
                tracing::info!("{service}")
            }

            // for file in directory.fi
        }

        Ok(())
    }

    if let Err(error) = actual_main() {
        ::tracing::error!("{error:?}");
        std::process::exit(1);
    }
}
