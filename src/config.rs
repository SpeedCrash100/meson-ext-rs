use std::{
    collections::HashMap,
    env,
    ffi::OsString,
    path::{Path, PathBuf},
    process::Command,
};

use semver::Version;

use crate::{Error, Result};

/// The configuration for the Meson build containing executable to run to build the project.
/// and options to pass into it.
///
#[derive(Debug, Clone)]
pub struct Config {
    meson_path: PathBuf,
    meson_version: Version,

    native_file: Option<PathBuf>,
    cross_file: Option<PathBuf>,
    out_path: Option<PathBuf>,

    options: HashMap<String, String>,

    /// Build profile: see `--buildtype` in the Meson documentation.
    profile: Option<String>,
}

impl Config {
    /// Find the system-wide Meson installation.
    ///
    /// See [`crate::find_meson`]
    pub fn find_system_meson() -> Result<Self> {
        let meson = Self::find_meson_in_system()?;
        let version = Self::get_version_of_meson(&meson)?;
        Ok(Self {
            meson_path: meson,
            meson_version: version,

            native_file: None,
            cross_file: None,
            out_path: None,

            options: HashMap::new(),

            profile: None,
        })
    }

    /// Gets meson version
    pub fn meson_version(&self) -> String {
        format!("{}", self.meson_version)
    }

    /// Sets the native file path for meson build
    pub fn set_native_file(&mut self, file: &Path) {
        self.native_file = Some(file.to_owned());
    }

    /// Sets the cross file path for meson build
    pub fn set_cross_file(&mut self, file: &Path) {
        self.cross_file = Some(file.to_owned());
    }

    /// Sets the output path for meson build
    ///
    /// There will be a `build` folder in the output path.
    pub fn set_out_path(&mut self, path: &Path) {
        self.out_path = Some(path.to_owned());
    }

    /// Sets the meson build option
    ///
    /// If options exists, it will be overwritten.
    pub fn set_option(&mut self, key: &str, value: &str) {
        self.options.insert(key.to_string(), value.to_string());
    }

    /// Set the meson build profile passed by `--buildtype` argument
    pub fn set_profile(&mut self, profile: &str) {
        self.profile = Some(profile.to_string());
    }

    /// Get the path of the build directory.
    pub fn build_dir(&self) -> PathBuf {
        self.out_path().join("build")
    }

    /// Get the path of the install directory.
    pub fn install_dir(&self) -> PathBuf {
        self.out_path().join("install")
    }

    fn is_configured(&self) -> bool {
        self.build_dir().join("build.ninja").exists()
    }

    fn configure(&self, source_dir: &Path) -> Result<()> {
        if self.is_configured() {
            return Ok(());
        }

        let build_dir = self.build_dir();
        std::fs::create_dir_all(&build_dir)?;

        let mut args: Vec<String> = vec!["setup".to_string()];

        let profile = self.profile();
        if !profile.is_empty() {
            args.extend(["--buildtype".to_string(), profile.to_string()]);
        } else {
            println!("cargo:info=profile is empty, ignoring profile option.");
        }

        let options = self
            .options
            .iter()
            .map(|(key, value)| format!("-D{}={}", key, value));

        args.extend(options);

        // Switch to OsString when dealing with paths
        let mut os_args: Vec<OsString> = args.into_iter().map(|x| OsString::from(x)).collect();

        // Native file
        if let Some(ref native_file) = self.native_file {
            os_args.extend([OsString::from("--native-file"), native_file.into()]);
        }

        // Cross file
        if let Some(ref cross_file) = self.cross_file {
            os_args.extend([OsString::from("--cross-file"), cross_file.into()]);
        }

        // Install prefix
        os_args.extend([OsString::from("--prefix"), self.install_dir().into()]);

        // Finally, source directory
        os_args.extend([source_dir.as_os_str().to_os_string()]);

        let mut command = Command::new(self.meson_path.clone());
        command.current_dir(source_dir);
        command.args(os_args);

        let status = command.status()?;
        if !status.success() {
            return match status.code() {
                Some(code) => Err(Error::MesonConfiguredUnsuccessfully(code)),
                None => Err(Error::MesonExitedBySignal),
            };
        }

        Ok(())
    }

    /// Start a new build process for the meson project in `source_dir`
    pub fn build(self, source_dir: &Path) -> Result<()> {
        self.configure(source_dir)?;

        let out_path = self.out_path();
        let build_dir = out_path.join("build");
        let install_dir = out_path.join("install");

        std::fs::create_dir_all(&build_dir)?;
        std::fs::create_dir_all(&install_dir)?;

        let mut build_command = Command::new(self.meson_path.clone());
        build_command.current_dir(source_dir);
        build_command.arg("build");
        build_command.arg("-C");
        build_command.arg(&build_dir);

        let status = build_command.status()?;
        if !status.success() {
            return match status.code() {
                Some(code) => Err(Error::MesonBuildUnsuccessfully(code)),
                None => Err(Error::MesonExitedBySignal),
            };
        }

        let mut install_command = Command::new(self.meson_path.clone());
        install_command.current_dir(source_dir);
        install_command.arg("install");
        install_command.arg("-C");
        install_command.arg(&build_dir);

        let status = install_command.status()?;
        if !status.success() {
            return match status.code() {
                Some(code) => Err(Error::MesonBuildUnsuccessfully(code)),
                None => Err(Error::MesonExitedBySignal),
            };
        }

        Ok(())
    }

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

    fn find_meson_target_specific_env() -> Option<PathBuf> {
        let target = env::var("TARGET").ok()?;
        let target_upper_case = target.to_uppercase().replace("-", "_");
        let target_specific_env = format!("MESON_{target_upper_case}");
        env::var_os(target_specific_env.as_str()).map(|x| x.into())
    }

    fn find_meson_in_system() -> Result<PathBuf> {
        if let Some(meson) = Self::find_meson_target_specific_env() {
            return Ok(meson);
        }

        if let Some(meson) = env::var_os("MESON") {
            return Ok(meson.into());
        }

        return Ok("meson".into());
    }

    fn out_path(&self) -> PathBuf {
        if let Some(path) = &self.out_path {
            path.to_owned()
        } else {
            let out_path = env::var_os("OUT_DIR")
                .expect("OUT_DIR is not set. Are you running outside of build.rs?");
            out_path.into()
        }
    }

    fn profile(&self) -> &str {
        match self.profile {
            Some(ref profile) => &profile,
            None => match env::var("PROFILE").unwrap().as_str() {
                "debug" => "debug",
                "release" => "release",
                profile => {
                    println!(
                        "cargo:warning=PROFILE '{profile}' is unknown.
                        Using release as default. Please override profile using set_profile"
                    );
                    "release"
                }
            },
        }
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
