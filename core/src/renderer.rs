use std::collections::HashMap;

use regex::Regex;
use serde::Deserialize;

use crate::{
    errors::RenderError,
    models::{
        config::{CommandConfig, EnvConfig, RegexConfig, TemplateConfig, WrapConfig},
        node::Node,
    },
};

pub struct CommandLatexConverter<'a> {
    pub configs: &'a HashMap<String, CommandConfig>,
}

impl<'a> CommandLatexConverter<'a> {
    pub fn compile_command_into_latex(&self, node: &Node) -> Result<String, RenderError> {
        match node {
            Node::Root { children, .. } => {
                let parts: Result<Vec<String>, RenderError> = children
                    .iter()
                    .map(|c| self.compile_command_into_latex(c))
                    .collect();
                Ok(parts?.join(""))
            }
            Node::Command {
                name,
                config_key,
                children,
                captures,
                ..
            } => match self.configs.get(config_key) {
                Some(config) => match config {
                    CommandConfig::Template(t) => self.format_template(name, children, t),
                    CommandConfig::Env(c) => self.format_environment(c, children),
                    CommandConfig::Regex(c) => self.format_regex(captures.clone(), c),
                    CommandConfig::Wrap(c) => self.format_wrap(c, children),
                },
                None => Err(RenderError::UnknownCommand(name.clone())),
            },
            Node::Leaf { content: text, .. } => {
                let mut text = text.to_string();
                replace_leaf_mut(&mut text)?;
                Ok(text)
            }
        }
    }
    fn format_environment(
        &self,
        config: &EnvConfig,
        children: &[Node],
    ) -> Result<String, RenderError> {
        let mut command = String::new();
        // command.push('\n');
        if let Some(s) = &config.output_prefix {
            command.push_str(s);
        }
        command.push_str("\\begin{");
        command.push_str(&config.env_name);
        command.push('}');
        // command.push('\n');
        let line_prefix = config.line_prefix.as_deref().unwrap_or("");
        let line_suffix = config.line_suffix.as_deref().unwrap_or("");
        let body = children
            .iter()
            .map(|child| match child {
                Node::Leaf { content, .. } => {
                    // dbg!(&config.replacements);
                    let converted = content.clone().replace(" ", &config.col_separator);
                    Ok(converted)
                }
                _ => self.compile_command_into_latex(child),
            })
            .map(|child| Ok(format!("{}{}{}", line_prefix, child?, line_suffix)))
            .collect::<Result<Vec<_>, RenderError>>()?
            .join(&config.row_separator); //改行削除した
        command.push_str(&body);
        // command.push('\n');
        command.push_str("\\end{");
        command.push_str(&config.env_name);
        command.push('}');
        if let Some(s) = &config.output_suffix {
            command.push_str(s);
        }
        // command.push('\n');
        Ok(command)
    }

    fn format_wrap(
        &self,
        config: &WrapConfig,
        children: &[Node],
    ) -> Result<String, RenderError> {
        let body = children
            .iter()
            .map(|child| self.compile_command_into_latex(child))
            .collect::<Result<Vec<_>, RenderError>>()?
            .join(&config.row_separator);
        Ok(format!("{}{}{}", config.prefix, body, config.suffix))
    }

    fn format_template(
        &self,
        name: &str,
        children: &[Node],
        config: &TemplateConfig,
    ) -> Result<String, RenderError> {
        let mut template = config.template.clone();

        let required = config.args_count;
        if children.len() != required {
            return Err(RenderError::MismatchArguments {
                command: name.to_string(),
                expected: required,
                found: children.len(),
            });
        }
        for (i, child) in children.iter().enumerate() {
            // $0, $1, $2... を探して置換
            let placeholder = format!("${}", i);
            let replacement = self.compile_command_into_latex(child)?;
            template = template.replace(&placeholder, &replacement);
        }

        Ok(template)
    }

    fn format_regex(
        &self,
        captures: Option<Vec<String>>,
        config: &RegexConfig,
    ) -> Result<String, RenderError> {
        let mut template = config.template.clone();
        let captures = captures.unwrap_or_default();
        let placeholder = Regex::new(r"\$[0-9]+").unwrap();
        let placeholder_count = placeholder.find_iter(&template).count();
        if captures.len() != placeholder_count {
            return Err(RenderError::MismatchTemplate {
                template: template.to_string(),
                expected: placeholder_count,
                found: captures.len(),
            });
        }
        // あえて大きい数字から見ることで$10を$1と誤認することを防ぐ
        for i in (0..captures.len()).rev() {
            // $0, $1, $2... を探して置換
            let placeholder = format!("${}", i + 1); //$1から
            let replacement = captures.get(i).expect("ensureで存在確認済み");
            template = template.replace(&placeholder, replacement);
        }

        Ok(template)
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct Replacement {
    pub pattern: String,
    pub to: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ReplacementsConfig {
    pub replacements: Vec<Replacement>,
}

const DEFAULT_REPLACEMENTS_STR: &str = include_str!("../replacements.toml");
fn replace_leaf_mut(leaf_str: &mut String) -> Result<(), RenderError> {
    let config: ReplacementsConfig =
        toml::from_str(DEFAULT_REPLACEMENTS_STR).map_err(|e| RenderError::Toml(e))?;
    for Replacement { pattern, to } in &config.replacements {
        let regex_pattern = Regex::new(pattern).map_err(|e| RenderError::Regex { source: e })?;
        *leaf_str = regex_pattern.replace_all(leaf_str, to).to_string();
    }
    Ok(())
}
