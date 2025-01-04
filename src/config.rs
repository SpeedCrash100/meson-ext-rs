use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

use semver::Version;

use crate::{Error, Result};

/// The configuration for the Meson build containing executable to run to build the project.
/// and options to pass into it.
///
pub struct Config {
    meson_path: PathBuf,
    meson_version: Version,
}

impl Config {
    /// Returns the version of Meson installed on this system.
    fn get_version_of_meson(meson_path: impl AsRef<Path>) -> Result<Version> {
        let mut command = Command::new(meson_path.as_ref());

        command.arg("--version");

        let output = command.output()?;

        if !output.status.success() {
            match output.status.code() {
                Some(code) => return Err(Error::MesonExitedUnsuccessfully(code)),
                None => return Err(Error::MesonExitedBySignal),
            }
        }

        let version_raw = core::str::from_utf8(&output.stdout)?.trim();
        let version = Version::parse(version_raw)?;

        Ok(version)
    }

    fn find_meson_in_system() -> Result<PathBuf> {
        let Ok(target) = env::var("TARGET") else {
            return Ok("meson".into());
        };

        let target_upper_case = target.to_uppercase().replace("-", "_");
        let target_specific_env = format!("MESON_{target_upper_case}");
        if let Some(meson) = env::var_os(target_specific_env.as_str()) {
            return Ok(meson.into());
        }

        return Ok("meson".into());
    }

    /// Find the system-wide Meson installation.
    ///
    /// See [`crate::find_meson`]
    pub fn find_system_meson() -> Result<Self> {
        let meson = Self::find_meson_in_system()?;
        let version = Self::get_version_of_meson(&meson)?;
        Ok(Self {
            meson_path: meson,
            meson_version: version,
        })
    }

    /// Gets meson version
    pub fn meson_version(&self) -> String {
        format!("{}", self.meson_version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_version_of_meson() {
        let meson_path = "meson";
        let _version =
            Config::get_version_of_meson(&meson_path).expect("Failed to get Meson version");
    }
}
