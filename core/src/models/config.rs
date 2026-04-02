use regex::Regex;
use serde::Deserialize;

use crate::errors::ConfigError;
// RAW_Command_CONFIG
#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type")] // "type" フィールドの値で分岐
pub enum CommandConfigRaw {
    #[serde(rename = "Template")]
    Template(TemplateConfigRaw),

    #[serde(rename = "Regex")]
    Regex(RegexConfigRaw),

    #[serde(rename = "Environment")]
    Env(EnvConfigRaw),

    #[serde(rename = "Wrap")]
    Wrap(WrapConfigRaw),
}

#[derive(Debug, Deserialize, Clone)]
pub struct TemplateConfigRaw {
    pub pattern: String,
    pub template: String,
    pub args_count: usize,
    pub alias: Option<Vec<String>>,
    pub completion_label: Option<String>,
    pub completion_template: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RegexConfigRaw {
    pub pattern: String,
    pub template: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct WrapConfigRaw {
    pub pattern: String,
    pub prefix: String,
    pub suffix: String,
    #[serde(default)]
    pub row_separator: String,
    pub alias: Option<Vec<String>>,
    pub completion_label: Option<String>,
    pub completion_template: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct EnvConfigRaw {
    pub pattern: String,
    pub env_name: String,
    pub output_prefix: Option<String>,
    pub output_suffix: Option<String>,
    pub line_prefix: Option<String>,
    pub line_suffix: Option<String>,
    pub alias: Option<Vec<String>>,
    #[serde(default)] // 何もなければString::new()つまり""が入る
    pub row_separator: String,
    #[serde(default)] // 何もなければString::new()つまり""が入る
    pub col_separator: String,
    pub completion_label: Option<String>,
    pub completion_template: Option<String>,
}

// Command_config_Validated
#[derive(Debug, Clone)]
pub enum CommandConfig {
    Template(TemplateConfig),
    Regex(RegexConfig),
    Env(EnvConfig),
    Wrap(WrapConfig),
}

#[derive(Debug, Clone)]
pub struct TemplateConfig {
    pub pattern: Regex,
    pub template: String,
    pub args_count: usize,
    pub alias: Vec<Regex>,
    pub completion_label: Option<String>,
    pub completion_template: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RegexConfig {
    pub pattern: Regex,
    pub template: String,
}

#[derive(Debug, Clone)]
pub struct WrapConfig {
    pub pattern: Regex,
    pub prefix: String,
    pub suffix: String,
    pub row_separator: String,
    pub alias: Vec<Regex>,
    pub completion_label: Option<String>,
    pub completion_template: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EnvConfig {
    pub pattern: Regex,
    pub env_name: String,
    pub output_prefix: Option<String>,
    pub output_suffix: Option<String>,
    pub line_prefix: Option<String>,
    pub line_suffix: Option<String>,
    pub alias: Vec<Regex>,
    pub row_separator: String,
    pub col_separator: String,
    pub completion_label: Option<String>,
    pub completion_template: Option<String>,
}

impl CommandConfigRaw {
    pub fn validate(self, name: &str) -> Result<CommandConfig, ConfigError> {
        match self {
            CommandConfigRaw::Template(c) => Ok(CommandConfig::Template(TemplateConfig {
                pattern: compile_regex(&c.pattern, &format!("{}.pattern", name))?,
                template: c.template,
                args_count: c.args_count,
                alias: compile_regex_all(c.alias.unwrap_or_default(), |i| {
                    format!("{}.alias[{}]", name, i)
                })?,
                completion_label: c.completion_label,
                completion_template: c.completion_template,
            })),
            CommandConfigRaw::Regex(c) => Ok(CommandConfig::Regex(RegexConfig {
                pattern: compile_regex(&c.pattern, &format!("{}.pattern", name))?,
                template: c.template,
            })),
            CommandConfigRaw::Wrap(c) => Ok(CommandConfig::Wrap(WrapConfig {
                pattern: compile_regex(&c.pattern, &format!("{}.pattern", name))?,
                prefix: c.prefix,
                suffix: c.suffix,
                row_separator: c.row_separator,
                alias: compile_regex_all(c.alias.unwrap_or_default(), |i| {
                    format!("{}.alias[{}]", name, i)
                })?,
                completion_label: c.completion_label,
                completion_template: c.completion_template,
            })),
            CommandConfigRaw::Env(c) => Ok(CommandConfig::Env(EnvConfig {
                pattern: compile_regex(&c.pattern, &format!("{}.pattern", name))?,
                env_name: c.env_name,
                output_prefix: c.output_prefix,
                output_suffix: c.output_suffix,
                line_prefix: c.line_prefix,
                line_suffix: c.line_suffix,
                alias: compile_regex_all(c.alias.unwrap_or_default(), |i| {
                    format!("{}.alias[{}]", name, i)
                })?,
                row_separator: c.row_separator,
                col_separator: c.col_separator,
                completion_label: c.completion_label,
                completion_template: c.completion_template,
            })),
        }
    }
}

fn compile_regex(raw_alias: &str, field_name: &str) -> Result<Regex, ConfigError> {
    //Ok(Regex::...()?)はOkと?で相殺されるのでけておっけー
    Regex::new(&format!("^{}$", raw_alias)).map_err(|e| ConfigError::Regex {
        field_name: field_name.into(),
        source: e,
    })
}

fn compile_regex_all<F: Fn(usize) -> String>(
    raw_alias_vec: Vec<String>,
    with_field_name: F,
) -> Result<Vec<Regex>, ConfigError> {
    raw_alias_vec
        .iter()
        .enumerate() // インデックスを取得
        .map(|(i, raw)| compile_regex(raw, &with_field_name(i)))
        .collect::<Result<Vec<_>, ConfigError>>()
}

// ----------------------------------
// ReplacementsConfig

#[derive(Debug, Deserialize, Clone)]
pub struct Replacement {
    pub pattern: String,
    pub to: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ReplacementsConfig {
    pub replacements: Vec<Replacement>,
}
