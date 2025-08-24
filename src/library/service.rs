//! Contains the definition of a service in [`Service`] as well
//! as all data and functions associated with processes.

#[derive(Debug, ::serde::Deserialize)]
pub struct Service {
    meta: Meta,
    id: String,
    start: Start,
}

impl PartialEq for Service {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Service {}

impl std::hash::Hash for Service {
    fn hash<H: std::hash::Hasher>(&self, hasher: &mut H) {
        self.id.hash(hasher);
    }
}

impl Service {
    /// TODO
    pub fn id(&self) -> &String {
        &self.id
    }

    /// TODO
    pub fn start(&self) -> &Start {
        &self.start
    }

    /// TODO
    pub fn serde_from_slice(slice: &Vec<u8>, path: &std::path::Path) -> ::anyhow::Result<Self> {
        use ::anyhow::Context as _;

        ::serde_yml::from_slice(&slice).context(format!(
            "Could not parse service definition in '{}'",
            path.display()
        ))
    }
}

/// TODO
#[derive(Debug, ::serde::Deserialize, ::serde::Serialize)]
struct Meta {
    /// TODO
    #[serde(deserialize_with = "deserialize::semver_version")]
    version: ::semver::Version,
}

/// TODO
#[derive(Debug, ::serde::Deserialize)]
pub struct Start {
    /// TODO
    #[serde(flatten)]
    _command_and_arguments: BasicCommand,
    /// TODO
    pub dependencies: Option<Vec<String>>,
}

/// TODO
#[derive(Debug, ::serde::Deserialize)]
struct BasicCommand {
    /// TODO
    command: String,
    /// TODO
    arguments: Option<Vec<String>>,
}

mod deserialize {
    //! Contains deserializers for non-standard types

    // /// Parse a [`std::time::Duration`] via [`::humantime`] from a [`String`]
    // pub fn option_humantime_duration<'de, D>(
    //     deserializer: D,
    // ) -> Result<Option<std::time::Duration>, D::Error>
    // where
    //     D: ::serde::Deserializer<'de>,
    // {
    //     let deserialized_string = <String as ::serde::Deserialize>::deserialize(deserializer)?;
    //     match ::humantime::parse_duration(&deserialized_string) {
    //         Ok(duration) => Ok(Some(duration)),
    //         Err(error) => Err(::serde::de::Error::custom(error)),
    //     }
    // }

    /// Parse a [`::semver::Version`] from a [`String`]
    pub fn semver_version<'de, D>(deserializer: D) -> Result<::semver::Version, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        let deserialized_string = <&str as ::serde::Deserialize>::deserialize(deserializer)?;
        match ::semver::Version::parse(deserialized_string) {
            Ok(version) => Ok(version),
            Err(error) => Err(::serde::de::Error::custom(error)),
        }
    }
}
