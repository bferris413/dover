use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub uses: ElementConfig,
    pub structs: ElementConfig,
    pub enums: ElementConfig,
    pub traits: ElementConfig,
    pub functions: ElementConfig,
    pub impls: ElementConfig,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ElementConfig {
    pub show: bool,
}

impl Default for ElementConfig {
    fn default() -> Self {
        Self { show: true }
    }
}

impl Config {
    pub fn path() -> Result<PathBuf> {
        let project_dirs = ProjectDirs::from("", "", "dover")
            .context("Could not determine the platform configuration directory")?;
        Ok(project_dirs.config_dir().join("dover.toml"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::path()?;
        Self::load_from(&path)
    }

    fn load_from(path: &Path) -> Result<Self> {
        let contents = match fs::read_to_string(path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("Error reading config at {}", path.display()));
            }
        };

        toml::from_str(&contents)
            .with_context(|| format!("Error parsing config at {}", path.display()))
    }

    pub fn init() -> Result<PathBuf> {
        let path = Self::path()?;
        Self::init_at(&path)
    }

    /// Return the config path, creating a default config when it does not exist.
    pub fn ensure_exists() -> Result<PathBuf> {
        let path = Self::path()?;
        Self::ensure_exists_at(&path)
    }

    fn init_at(path: &Path) -> Result<PathBuf> {
        if path.exists() {
            bail!("Config already exists at {}", path.display());
        }

        Self::write_default_at(path)
    }

    fn ensure_exists_at(path: &Path) -> Result<PathBuf> {
        if path.exists() {
            return Ok(path.to_owned());
        }

        match Self::write_default_at(path) {
            Ok(path) => Ok(path),
            // Another process may have created the config after the existence check.
            Err(error)
                if error
                    .downcast_ref::<io::Error>()
                    .is_some_and(|error| error.kind() == io::ErrorKind::AlreadyExists) =>
            {
                Ok(path.to_owned())
            }
            Err(error) => Err(error),
        }
    }

    fn write_default_at(path: &Path) -> Result<PathBuf> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Error creating config directory at {}", parent.display())
            })?;
        }

        let contents = toml::to_string_pretty(&Self::default())
            .context("Error serializing the default config")?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .with_context(|| format!("Error creating config at {}", path.display()))?;
        file.write_all(contents.as_bytes())
            .with_context(|| format!("Error writing config at {}", path.display()))?;
        Ok(path.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn missing_elements_and_settings_use_visible_defaults() {
        let config: Config = toml::from_str(
            r#"
[enums]
show = false

[structs]
"#,
        )
        .unwrap();

        assert!(!config.enums.show);
        assert!(config.structs.show);
        assert!(config.uses.show);
        assert!(config.functions.show);
    }

    #[test]
    fn serialized_default_lists_every_supported_element() {
        let contents = toml::to_string_pretty(&Config::default()).unwrap();

        for element in ["uses", "structs", "enums", "traits", "functions", "impls"] {
            assert!(contents.contains(&format!("[{element}]")), "{contents}");
        }
        assert_eq!(contents.matches("show = true").count(), 6, "{contents}");
    }

    #[test]
    fn unknown_settings_are_rejected() {
        let error = toml::from_str::<Config>("[enums]\nvisible = true\n").unwrap_err();
        assert!(error.to_string().contains("unknown field `visible`"));
    }

    #[test]
    fn init_creates_a_complete_config_without_overwriting() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("dover-config-test-{}-{unique}", std::process::id()));
        let path = directory.join("dover.toml");

        assert_eq!(Config::init_at(&path).unwrap(), path);
        assert_eq!(Config::load_from(&path).unwrap(), Config::default());
        assert!(
            Config::init_at(&path)
                .unwrap_err()
                .to_string()
                .contains("already exists")
        );

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn ensure_exists_creates_a_default_and_preserves_an_existing_config() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "dover-config-ensure-test-{}-{unique}",
            std::process::id()
        ));
        let path = directory.join("dover.toml");

        assert_eq!(Config::ensure_exists_at(&path).unwrap(), path);
        assert_eq!(Config::load_from(&path).unwrap(), Config::default());

        let custom = "[enums]\nshow = false\n";
        fs::write(&path, custom).unwrap();
        assert_eq!(Config::ensure_exists_at(&path).unwrap(), path);
        assert_eq!(fs::read_to_string(&path).unwrap(), custom);

        fs::remove_dir_all(directory).unwrap();
    }
}
