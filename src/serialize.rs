#[derive(Debug, ::serde::Deserialize, ::serde::Serialize)]
struct Meta {
    version: String,
}

#[derive(Debug, PartialEq, ::serde::Deserialize, ::serde::Serialize)]
struct Command {
    command: String,
    arguments: Option<Vec<String>>,
}

#[derive(Debug, ::serde::Deserialize, ::serde::Serialize)]
struct Start {
    #[serde(flatten)]
    command: Command,
    user: String,
    after: Option<Vec<String>>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        deserialize_with = "Service::deserialize_option_humantime"
    )]
    delay: Option<std::time::Duration>,
}

#[derive(Debug, ::serde::Deserialize, ::serde::Serialize)]
struct Restart {
    command: Option<Command>,
    strategy: String,
    attempts: u64,
}

#[derive(Debug, PartialEq, ::serde::Deserialize, ::serde::Serialize)]
#[serde(untagged, rename_all = "lowercase")]
enum CommandOrSignal {
    Command {
        #[serde(flatten)]
        command: Command,
    },
    Signal {
        signal: String,
    },
}

#[derive(Debug, ::serde::Deserialize, ::serde::Serialize)]
struct Termination {
    #[serde(flatten)]
    command_or_signal: CommandOrSignal,

    before: Option<Vec<String>>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        deserialize_with = "Service::deserialize_option_humantime"
    )]
    delay: Option<std::time::Duration>,
}

#[derive(Debug, ::serde::Deserialize, ::serde::Serialize)]
struct Environment {
    clear: Option<bool>,
    variables: Option<::std::collections::HashMap<String, String>>,
}

#[derive(Debug, ::serde::Deserialize, ::serde::Serialize)]
pub struct Log {
    stdin: Option<std::path::PathBuf>,
    stdout: Option<std::path::PathBuf>,
    stderr: Option<std::path::PathBuf>,
}

#[derive(Debug, ::serde::Deserialize, ::serde::Serialize)]
pub struct Diagnosis {
    level5: Option<Command>,
    level4: Option<Command>,
    level3: Option<Command>,
    level2: Option<Command>,
    level1: Option<Command>,
}

#[derive(Debug, ::serde::Deserialize, ::serde::Serialize)]
pub struct Service {
    meta: Meta,
    id: String,
    start: Start,
    restart: Restart,
    termination: Option<Termination>,
    environment: Option<Environment>,
    log: Option<Log>,
    diagnosis: Diagnosis,
}

impl std::fmt::Display for Service {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Service {} {{
{}}}",
            self.id,
            ::serde_yml::to_string(self).unwrap()
        )
    }
}

#[derive(Debug, ::thiserror::Error)]
pub struct Error {
  service_name: String,
  exit_status: i32
}

impl ::std::fmt::Display for Error {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      write!(f, "Service {} failed (exit status: {})", self.service_name, self.exit_status)
  }
}

impl Service {
    fn deserialize_option_humantime<'de, D>(
        deserializer: D,
    ) -> Result<Option<std::time::Duration>, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        let deserialized_string = <String as ::serde::Deserialize>::deserialize(deserializer)?;
        match ::humantime::parse_duration(&deserialized_string) {
            Ok(duration) => Ok(Some(duration)),
            Err(error) => Err(::serde::de::Error::custom(error)),
        }
    }

    pub async fn run(self) -> Result<(), Error> {
        let mut command = ::tokio::process::Command::new(&self.start.command.command);
        if let Some(ref arguments) = self.start.command.arguments {
            command.args(arguments);
        }
        let mut child = command.spawn().unwrap();
        let x = child.wait().await.unwrap();
        if x.success() {
          Ok(())
        } else {
          tracing::warn!("Exit status: {x}");
          Err(Error {
            service_name: self.id,
            exit_status: x.code().unwrap_or(1)
          })
        }
    }
}
