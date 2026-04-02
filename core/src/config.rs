use std::{collections::HashMap, fs, path::Path};

use regex::Regex;

use crate::{
    errors::ConfigError,
    models::config::{
        CommandConfig, CommandConfigRaw, EnvConfig, ReplacementsConfig, TemplateConfig, WrapConfig,
    },
};

const DEFAULT_CONFIG_STR: &str = include_str!("../commands.toml");
pub fn load_command_config(
    path: Option<&Path>,
) -> Result<HashMap<String, CommandConfig>, ConfigError> {
    // TODO: 重複時に警告するプロセスを作成
    let content = match path {
        Some(p) => fs::read_to_string(p).map_err(|_| {
            ConfigError::CommandConfigFileNotFound(
                dunce::canonicalize(p)
                    .expect("canonicalize error")
                    .as_os_str()
                    .to_str()
                    .unwrap()
                    .to_string(),
            )
        })?,
        None => DEFAULT_CONFIG_STR.to_string(),
    };
    //|e| ConfigError::Toml(e) = ConfigError::Toml
    let map: HashMap<String, CommandConfigRaw> =
        toml::from_str(&content).map_err(ConfigError::Toml)?;
    let validated_map = map
        .into_iter()
        .map(|(key, value)| Ok((key.clone(), value.validate(&key)?)))
        .collect::<Result<HashMap<_, _>, ConfigError>>()?;
    let mut map_extended: HashMap<String, CommandConfig> = HashMap::new();
    for (name, config) in validated_map {
        let aliases: &Vec<Regex> = match &config {
            CommandConfig::Template(t) => &t.alias,
            CommandConfig::Env(e) => &e.alias,
            CommandConfig::Wrap(w) => &w.alias,
            CommandConfig::Regex(_) => &Vec::new(),
        };
        for (i, alias) in aliases.iter().enumerate() {
            let aliased_config = match &config {
                CommandConfig::Template(t) => CommandConfig::Template(TemplateConfig {
                    pattern: alias.clone(),
                    completion_label: None,
                    completion_template: None,
                    ..t.clone()
                }),
                CommandConfig::Env(e) => CommandConfig::Env(EnvConfig {
                    pattern: alias.clone(),
                    completion_label: None,
                    completion_template: None,
                    ..e.clone()
                }),
                CommandConfig::Wrap(w) => CommandConfig::Wrap(WrapConfig {
                    pattern: alias.clone(),
                    completion_label: None,
                    completion_template: None,
                    ..w.clone()
                }),
                CommandConfig::Regex(_) => unreachable!(),
            };
            map_extended.insert(name.clone() + "_alias_" + &i.to_string(), aliased_config);
        }
        map_extended.insert(name, config);
    }
    Ok(map_extended)
}

pub fn load_replacements_config(path: Option<&Path>) -> Result<ReplacementsConfig, ConfigError> {
    let replacements_raw = match path {
        Some(p) => fs::read_to_string(p).map_err(|_| {
            ConfigError::ReplacementsFileNotFound(
                dunce::canonicalize(p)
                    .expect("canonicalize error")
                    .as_os_str()
                    .to_str()
                    .unwrap()
                    .to_string(),
            )
        })?,
        None => "replacements=[]".to_string(), // return empty
    };
    let config: ReplacementsConfig =
        toml::from_str(&replacements_raw).map_err(ConfigError::Toml)?;
    Ok(config)
}
