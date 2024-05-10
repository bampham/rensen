use serde::{Serialize, Deserialize};
use serde_json;
use serde_yaml;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::io::{prelude::*, self, Write, Read};
use std::fmt;

use crate::traits;
use traits::FileSerializable;

use crate::logging;
use logging::{log_trap, Trap};

#[derive(Serialize, Deserialize)]
pub struct HostConfig {
    pub user: String,
    pub identifier: String, // ip address or domain name
    pub port: Option<u16>, // default: 22
    pub key_path: Option<PathBuf>, // default: "$HOME/.ssh/id_rsa"
    pub source: PathBuf,
    pub destination: PathBuf,
    pub frequency_hrs: Option<f32>, // default: 24.0
}

#[derive(Serialize, Deserialize)]
pub struct Host {
    pub hostname: String,
    pub config: HostConfig
}

pub struct Settings {
    pub hosts: Vec<Host>,
}

impl HostConfig {

    pub fn from(
        user: String,
        identifier: String,
        port: u16,
        key_path: PathBuf,
        source: PathBuf,
        destination: PathBuf,
        frequency_hrs: f32,
        ) -> Self {
        Self {
            user,
            identifier,
            port: Some(port),
            key_path: Some(key_path),
            source,
            destination,
            frequency_hrs: Some(frequency_hrs),
        }
    }
}


impl Settings {
    pub fn new(hosts: Vec<Host>) -> Self {
        Self { hosts }  
    }

    pub fn verify_syntax_json(file_path: &Path) -> std::io::Result<()> {
        let mut file = File::open(file_path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let _: Vec<HostConfig> = match serde_json::from_str(&contents) {
            Ok(v) => v,
            Err(err) => {
                return Err(err.into());
            }
        };

        Ok(())
    }


    
}

impl FileSerializable for Settings {
    fn serialize_json(&self, file_path: &Path) -> std::io::Result<()> {
        let mut file = File::create(file_path)?;
        let json_str = serde_json::to_string_pretty(&self.hosts)?;
        write!(file, "{}", json_str)?;
        Ok(())
    }

    fn deserialize_json(file_path: &Path) -> std::io::Result<Self> {
        let mut file = File::open(file_path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let hosts: Vec<Host> = serde_json::from_str(&contents)?;
        Ok(Self {hosts})
    }

    // yaml
    fn serialize_yaml(&self, file_path: &Path) -> std::io::Result<()> {
        let mut file = File::create(file_path)?;
        let yaml_str = serde_yaml::to_string(&self.hosts)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;

        let formatted_yaml = serde_yaml::to_string(&serde_yaml::from_str::<serde_yaml::Value>(&yaml_str)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))? 
        ).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;

        file.write_all(formatted_yaml.as_bytes())?;
        Ok(())
    }

    fn deserialize_yaml(file_path: &Path) -> std::io::Result<Self> {
        let mut file = File::open(file_path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let hosts: Vec<Host> = serde_yaml::from_str(&contents)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        Ok(Self { hosts })
    }
}
