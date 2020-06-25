use semver::{Version, VersionReq};
use serde_json::{from_str, Value};
use std::{
    fs::File,
    io::{BufRead, BufReader},
};
use walkdir::WalkDir;

fn main() {
    let target_version = Version::parse("0.3.9").unwrap();
    let bad_version = Version::parse("0.2.8").unwrap();
    for entry in WalkDir::new("crates.io-index")
        .into_iter()
        .filter_entry(|e| e.file_name() != ".git" && e.file_name() != "config.json")
    {
        let entry = entry.unwrap();
        let name = entry.file_name().to_str().unwrap();
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
        println!("{}:{}", name, crate_version);
    }
}
