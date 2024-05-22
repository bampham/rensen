use rensen_lib::logging::Trap; 
use rensen_lib::config::*;
use rensen_lib::traits::{YamlFile, JsonFile, Rsync};
use rensen_lib::backup::rsync::Sftp;
use rensen_lib::record::Record;
use rensen_lib::compiler::Compiler;

use console::Style;

use crate::utils::*;

use std::path::PathBuf;
use std::fs;

use console::style;

#[derive(PartialEq)]
pub enum BackupMethod {
    Full,
    Incremental,
}

#[derive(PartialEq)]
pub enum ActionType {
    AddHost,    // (1 arg)
    DeleteHost, // (1 arg)
    ModifyHost, // (1 arg)
    RunBackup,  // (2 arg)
    Compile,    // (2 arg)
    List,       // (1 arg)
    Help,       // (0 arg)
    Exit,       // (0 arg)
}

pub struct Action {
    pub action_type: ActionType,
    pub operands: Vec<String>,

    pub global_config: GlobalConfig,
}

impl Action {
    pub fn execute(&self) -> Result<(), Trap> {

        match self.action_type {
            ActionType::AddHost    => {
                self.add_host()?;
            },
            ActionType::DeleteHost=> {
                self.del_host()?;
            },
            ActionType::RunBackup  => {
                self.run_backup()?;
            },
            ActionType::Compile    => {
                self.compile_snapshot()?;
            },
            ActionType::List=> {
                self.list_snapshots()?;
            },
            ActionType::Help       => {
                self.print_help();
            }

            _ => (),
        }

        Ok(())
    }

    fn add_host(&self) -> Result<(), Trap> {

        let hosts_path = &self.global_config.hosts_path;

        let mut settings: Settings = Settings::deserialize_yaml(&hosts_path)
            .map_err(|err| Trap::FS(format!("Could not deserialize settings: {}", err)))?;

        // Read addr
        let identifier = get_input("addr: ")
            .map_err(|err| Trap::ReadInput(format!("Could not read input: {}", err)))?.trim().to_string();
        
        // Read Username 
        let user = get_input("user: ")
            .map_err(|err| Trap::ReadInput(format!("Could not read input: {}", err)))?.trim().to_string();

        // Read port
        let port = match get_input("port (press enter for 22): ") {
            Ok(input) => {
                if input.trim().is_empty() {
                    22
                }
                else {
                    match input.trim().parse::<u16>() {
                        Ok(port) => port,
                        Err(err) => {
                            return Err(
                                Trap::ReadInput(format!("Could not read input: {}", err))
                            );
                        }
                    }
                }
            }
            Err(err) => {
                println!("Failed to read input");
                return Err(Trap::ReadInput(format!("Could not read input: {}", err)));
            }
        };

        // Read key-path
        let key_path = get_input("ssh-key: ")
            .map_err(|err| Trap::ReadInput(format!("Could not read input: {}", err)))?
            .trim().to_string();

        // Read source directory
        let source = get_input("source: ")
            .map_err(|err| Trap::ReadInput(format!("Could not read input: {}", err)))?
            .trim().to_string();

        // Read destination/backup directory
        let destination = get_input("destination: ")
            .map_err(|err| Trap::ReadInput(format!("Could not read input: {}", err)))?
            .trim().to_string();

        // Read backup frequency
        let frequency_hrs = match get_input("backup frquency (hrs): ") {
            Ok(input) => {
                if input.trim().is_empty() {
                    24.0
                }
                else {
                    match input.trim().parse::<f32>() {
                        Ok(port) => port,
                        Err(err) => {
                            return Err(
                                Trap::ReadInput(format!("Could not convert to f32: {}", err))
                            );
                        }
                    }
                }
            }
            Err(err) => {
                println!("Failed to read input");
                return Err(Trap::ReadInput(format!("Could not read input: {}", err)));
            }

        };

        // Add Config to settings and serialize
        let hostconfig = HostConfig::from(user.to_string(), identifier.to_string(), port, PathBuf::from(key_path), PathBuf::from(source), PathBuf::from(destination), frequency_hrs);
        settings.hosts.push(Host { hostname: self.operands[0].clone(), config: hostconfig  });
        
        let _ = settings.serialize_yaml(hosts_path)
            .map_err(|err| Trap::FS(format!("Could not serialize yaml: {}", err)))?;

        Ok(())
    }

    fn del_host(&self) -> Result<(), Trap> {
        if self.operands.len() != 1 {
            return Err(
                Trap::InvalidInput(
                    String::from("Invalid arguments for action. Use `help` for more details")
                )
            );
        }
        
        let hostname = &self.operands[0];
        let hosts_path = &self.global_config.hosts_path;

        // Global host-settings for rensen
        let mut settings: Settings = Settings::deserialize_yaml(&hosts_path)
            .map_err(|err| Trap::FS(format!("Could not deserialize settings: {}", err)))?;
        
        // Removing host from the settings by extractin it's index
        for (i, host) in settings.hosts.iter().enumerate() {
            if host.hostname.to_owned() == hostname.to_owned() {
                settings.hosts.remove(i);
                break;
            }
        }

        // Writing it back to the file
        settings.serialize_yaml(&hosts_path)
            .map_err(|err| Trap::FS(format!("Could not serialize settings: {}", err)))?;

        println!("Deleted {}", hostname);

        Ok(())
    }

    fn compile_snapshot(&self) -> Result<(), Trap> {
        if self.operands.len() != 1 {
            return Err(
                Trap::InvalidInput(
                    String::from("Invalid arguments for action. Use `help` for more details")
                )
            );
        }

        let hosts_path = &self.global_config.hosts_path;
        let hostname = &self.operands[0];

        let settings: Settings = Settings::deserialize_yaml(hosts_path)
            .map_err(|err| Trap::FS(format!("Could not deserialize {:?}: {}", hosts_path, err)))?;

        let host_config = match settings.associated_config(&hostname) {
            Some(config) => config,
            None => return Err(Trap::InvalidInput(format!("hostname `{}` is not found", hostname)))
        };

        let mut snapshot = get_input("Snapshot: ")
            .map_err(|err| Trap::InvalidInput(format!("Could not read input: {:?}", err)))?;
        
        // Making it point to the record.json file if `latest` if given
        if snapshot.trim() == "latest" {
            snapshot = String::from("record");
        }

        let snapshot_record_path = self.global_config.backupping_path
            .join(host_config.identifier)
            .join(".records")
            .join(format!("{}.json", snapshot.trim()));

        /* Compiling snapshot */
        let mut compiler = Compiler::from(&snapshot_record_path)?;
        compiler.compile(&self.global_config.snapshots_path)?;
        let _ = compiler.cleanup();

        Ok(())
    }

    fn list(&self) -> Result<(), Trap> {
        if self.operands.len() != 2 {
            return Err(
                Trap::InvalidInput(
                    String::from("Invalid arguments for action. Use `help` for more details")
                )
            );
        }

        Ok(())
    }

    fn list_snapshots(&self) -> Result<(), Trap> {
        if self.operands.len() != 1 {
            return Err(
                Trap::InvalidInput(
                    String::from("Invalid arguments for action. Use `help` for more details")
                )
            );
        }

        let hosts_path = &self.global_config.hosts_path;
        let hostname = &self.operands[0];

        // Gettings the Settings
        let settings: Settings = Settings::deserialize_yaml(hosts_path)
            .map_err(|err| Trap::FS(format!("Could not deserialize {:?}: {}", hosts_path, err)))?;

        // Extracting the config for associated hostname
        let host_config = match settings.associated_config(&hostname) {
            Some(config) => config,
            None => return Err(Trap::InvalidInput(format!("hostname `{}` is not found", hostname)))
        };

        let dir_path = self.global_config.backupping_path
            .join(host_config.identifier)
            .join(".records")
        ;

        /* Reading directory contentens and formatting outputs */

        let entries = match fs::read_dir(&dir_path) {
            Ok(entries) => entries,
            Err(err) => return Err(Trap::FS(
                format!("Could not read directory at: `{:?}`: {}", dir_path, err)))
        };


        let style = console::Style::new();
        println!("{}\n", style.clone().bold().apply_to("Snapshots: "));

        for entry in entries {

            let entry = entry.unwrap();
            if let Some(file_stem) = entry.path().file_stem() {

                // Filtering out the record.json file
                if file_stem != "record" {
                    println!("->  {}", style.clone().bold().blue().apply_to(file_stem.to_str().unwrap()));
                }
            }

        }
        println!();

        Ok(())
    }

    fn run_backup(&self) -> Result<(), Trap> {
        if self.operands.len() != 2 {
            return Err(
                Trap::InvalidInput(
                    String::from("Invalid arguments for action. Use `help` for more details")
                )
            );
        }

        let hosts_path = &self.global_config.hosts_path;
        let hostname = &self.operands[0];

        // Opening the settings file for all hosts
        let settings: Settings = Settings::deserialize_yaml(hosts_path)
            .map_err(|err| Trap::FS(format!("Could not deserialize {:?}: {}", hosts_path, err)))?;

        let mut host_config = match settings.associated_config(&hostname) {
            Some(config) => config,
            None => return Err(Trap::InvalidInput(format!("hostname `{}` is not found", hostname)))
        };

        let record_path = host_config.destination
            .join(&host_config.identifier)
            .join(".records")
            .join("record.json")
        ;
        
        let record = Record::deserialize_json(&record_path)
            .map_err(|err| Trap::FS(format!("Could not read record {:?}: {}", record_path, err)))?;

        let mut sftp = Sftp::new(&mut host_config, &self.global_config, record, false);
        
        let backup_method: BackupMethod = match self.operands[1].to_lowercase().as_str() {
            "inc"         => BackupMethod::Incremental,
            "incremental" => BackupMethod::Incremental,
            "full"        => BackupMethod::Full,
            _             => return Err(Trap::InvalidInput(String::from("Invalid input")))
        };

        if backup_method == BackupMethod::Incremental {
            sftp.incremental = true;
        }

        sftp.backup()?;

        Ok(())
    }

    pub fn print_help(&self) {
        let style = Style::new();

        if self.operands.len() > 0 {
            match self.operands[0].to_lowercase().as_str() {
                "add"     => {
                    println!("a, add <hostname>     Enters host-adding interface.");
                    println!(
                        "Enters the host-adding interface where you are able to specify information about\nthen host which is going to be backupped.\n\n{} Remember to have a ssh-key generated in the path you specify, and also have thepublic key on the host machine.",
                    style.bold().red().apply_to("Note:"));
                },
                "del"     => {
                    println!("d, del <hostname>     Deletes host config.");
                    println!("Deletes the specified host's config from the configuration file located at probably in /etc/rensen or has specified path in /etc/rensen/rensen_config");
                },
                "modify"  => (),
                "run"     => (),
                "list"    => (),
                "compile" => (),
                _ => (),
            }

            return;
        }

        println!("h, ?, help                          Show this info.");
        println!("q, quit, exit                       Quits ctl.\n");

        println!("a,     add <hostname>               Enters host-adding interface.");
        println!("d,     del <hostname>               Deletes host config.");
        println!("m,  modify <hostname>               Enters modification interface.");
        println!("r,     run <hostname> <inc, full>   Runs backup for host based on what is specified in config."); 
        println!("l,    list <hostname>               Lists snapshots taken of host.");
        println!("c, compile <hostname>               Starts compilation interface.");
    }
}
