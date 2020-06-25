use semver::{Version, VersionReq};
use serde_json::{from_str, Value};
use std::{
    fs::{remove_dir_all, write, File},
    io::{BufRead, BufReader},
    process::{exit, Command, Stdio},
};
use walkdir::WalkDir;

fn main() {
    let target_version = Version::parse("0.3.9").unwrap();
    let bad_version = Version::parse("0.2.8").unwrap();
    let mut broken = Vec::new();
    for entry in WalkDir::new("crates.io-index")
        .into_iter()
        .filter_entry(|e| e.file_name() != ".git" && e.file_name() != "config.json")
    {
        let entry = entry.unwrap();
        let crate_name = entry.file_name().to_str().unwrap();
        if !entry.file_type().is_file() {
            continue;
        }
        let file = BufReader::new(File::open(entry.path()).unwrap());
        let mut needed_winapi = None;
        let mut crate_version: Option<Version> = None;
        for line in file.lines() {
            let line = line.unwrap();
            let json: Value = from_str(&line).unwrap();
            if json["yanked"].as_bool().unwrap() == true {
                continue;
            }
            let version = Version::parse(json["vers"].as_str().unwrap()).unwrap();
            if let Some(ref existing_version) = crate_version {
                if (version.is_prerelease()
                    && (!existing_version.is_prerelease() || existing_version > &version))
                    || (!version.is_prerelease()
                        && !existing_version.is_prerelease()
                        && existing_version > &version)
                {
                    continue;
                }
            }
            if let Some(winapi) = json["deps"]
                .as_array()
                .unwrap()
                .iter()
                .find(|x| x["name"] == "winapi")
            {
                crate_version = Some(version);
                needed_winapi = Some(winapi["req"].as_str().unwrap().to_owned());
            } else {
                crate_version = None;
                needed_winapi = None;
            }
        }
        let needed_version = match needed_winapi {
            Some(version) => VersionReq::parse(&version).unwrap(),
            _ => continue,
        };
        let crate_version = crate_version.unwrap();
        if !needed_version.matches(&target_version) {
            continue;
        }
        if needed_version.matches(&bad_version) {
            continue;
        }
        write(
            r"before\Cargo.toml",
            format!(
                r#"[package]
name = "wincheck-before"
version = "0.1.0"
edition = "2018"

[dependencies]
{} = "{}""#,
                crate_name, crate_version
            ),
        )
        .unwrap();
        write(
            r"after\Cargo.toml",
            format!(
                r#"[package]
name = "wincheck-after"
version = "0.1.0"
edition = "2018"

[patch.crates-io]
winapi = {{ git = "https://github.com/retep998/winapi-rs.git", branch = "0.3" }}

[dependencies]
{} = "{}""#,
                crate_name, crate_version
            ),
        )
        .unwrap();
        let _ = remove_dir_all(r"before\target");
        let _ = remove_dir_all(r"after\target");
        let before = Command::new("cargo")
            .arg("build")
            .current_dir("before")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let after = Command::new("cargo")
            .arg("build")
            .current_dir("after")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        match (before.status.success(), after.status.success()) {
            (true, true) => println!("{}:{} unchanged working", crate_name, crate_version),
            (true, false) => {
                println!("{}:{} was broken!!!", crate_name, crate_version);
                let before_output = format!(
                    "{}\n{}",
                    String::from_utf8_lossy(&before.stdout),
                    String::from_utf8_lossy(&before.stderr)
                );
                let after_output = format!(
                    "{}\n{}",
                    String::from_utf8_lossy(&after.stdout),
                    String::from_utf8_lossy(&after.stderr)
                );
                broken.push((
                    crate_name.to_owned(),
                    crate_version,
                    before_output,
                    after_output,
                ));
            }
            (false, true) => println!("{}:{} was magically fixed?!", crate_name, crate_version),
            (false, false) => println!("{}:{} unchanged failing", crate_name, crate_version),
        }
    }
    if broken.is_empty() {
        exit(0);
    }
    for (crate_name, crate_version, _, after) in broken {
        println!("Rust output from {}:{}", crate_name, crate_version);
        println!("{}", after);
    }
    exit(1);
}
