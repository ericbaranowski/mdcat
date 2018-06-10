// Copyright 2018 Sebastian Wiesner <sebastian@swsnr.de>

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at

// 	http://www.apache.org/licenses/LICENSE-2.0

// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![deny(warnings)]

#[macro_use]
extern crate failure;
extern crate json;
extern crate semver;
extern crate toml_edit;

use failure::{err_msg, Error};
use semver::Version;
use std::path::{Path, PathBuf};
use std::process::Command;
use toml_edit::Document;

fn get_workspace_root() -> Result<PathBuf, Error> {
    let output = Command::new("cargo")
        .arg("metadata")
        .arg("--format-version")
        .arg("1")
        .output()?
        .stdout;
    let stdout = std::str::from_utf8(&output)?;
    let metadata = json::parse(stdout)?;
    metadata["workspace_root"]
        .as_str()
        .map(Into::into)
        .ok_or(err_msg("Missing workspace root"))
}

fn read_manifest(path: &Path) -> Result<Document, Error> {
    let document = std::fs::read_to_string(path)?.parse::<Document>()?;
    Ok(document)
}

fn write_manifest(path: &Path, document: &Document) -> std::io::Result<()> {
    std::fs::write(path, document.to_string())
}

fn commit_all(workspace_root: &Path, message: &str) -> Result<(), Error> {
    let status = Command::new("git")
        .current_dir(workspace_root)
        .arg("commit")
        .arg("--all")
        .arg("--message")
        .arg(message)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format_err!(
            "Command git commit --all failed with status {}",
            status,
        ))
    }
}

fn make_tag(workspace_root: &Path, version: &Version, package_name: &str) -> Result<(), Error> {
    let status = Command::new("git")
        .current_dir(workspace_root)
        .arg("tag")
        .arg("--sign")
        .arg("--message")
        .arg(&format!("{} {}", package_name, version))
        .arg(&format!("{}-{}", package_name, version))
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format_err!(
            "Command git tag --sign failed with status {}",
            status,
        ))
    }
}

fn get_version(document: &Document) -> Result<Version, Error> {
    let value = document["package"]["version"]
        .as_str()
        .ok_or(err_msg("Version missing!"))?;
    let version = Version::parse(value)?;
    Ok(version)
}

fn set_version(document: &mut Document, version: &Version) {
    document["package"]["version"] = toml_edit::value(version.to_string());
}

fn update_lock(workspace_root: &Path, name: &str) -> Result<(), Error> {
    let status = Command::new("cargo")
        .current_dir(workspace_root)
        .arg("update")
        .arg("--package")
        .arg(name)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format_err!(
            "Command cargo update --package {} failed with status {}",
            name,
            status,
        ))
    }
}

fn make_release() -> Result<(), Error> {
    let workspace_root = get_workspace_root()?;
    let cargo_toml = workspace_root.join("Cargo.toml");
    let mut manifest = read_manifest(&cargo_toml)?;
    let package_name = manifest["package"]["name"]
        .as_str()
        .ok_or(err_msg("Package name missing!"))?
        .to_owned();
    let version = get_version(&manifest)?;

    if version.is_prerelease() {
        let mut next_version = version.clone();
        // TODO: Allow to bump different parts
        next_version.increment_minor();
        set_version(&mut manifest, &next_version);
        write_manifest(&cargo_toml, &manifest)?;
        update_lock(&workspace_root, &package_name)?;
        commit_all(&workspace_root, &format!("Release {}", next_version))?;
        make_tag(&workspace_root, &next_version, &package_name)?;
        Ok(())
    } else {
        Err(format_err!(
            "Cannot make release from final version: {}",
            version
        ))
    }
}

fn main() {
    match make_release() {
        Ok(_) => (),
        Err(error) => {
            eprintln!("Release failed: {}", error);
            std::process::exit(1);
        }
    }
}
