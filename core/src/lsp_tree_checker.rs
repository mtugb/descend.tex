use std::collections::HashMap;

use crate::{
    errors::{LintError, LintErrorKind},
    models::{
        config::{CommandConfig, EnvConfig, RegexConfig, TemplateConfig, WrapConfig},
        node::Node,
    },
};

// This function is for LSP
pub fn check_tree(
    current: Node,
    indent_unit: Option<usize>,
    configs: &HashMap<String, CommandConfig>,
    mut provided_environments: Vec<String>,
) -> Result<(), LintError> {
    let indent_unit = indent_unit.unwrap_or(4);
    match current {
        Node::Root { children, .. } => {
            for child in children {
                check_tree(
                    child,
                    Some(indent_unit),
                    configs,
                    provided_environments.clone(),
                )?;
            }
            return Ok(());
        }
        Node::Command {
            name,
            config_key,
            captures: _,
            children,
            line_num,
            leading_chars,
        } => {
            match configs.get(&config_key) {
                Some(config) => {
                    // コマンドだった
                    if let CommandConfig::Template(t) = config {
                        // Template commandだけ引数の個数チェックが必要
                        let expected = t.args_count;
                        let found = children.len();
                        if expected != found {
                            return Err(LintError {
                                line: line_num,
                                character: leading_chars as usize,
                                kind: crate::errors::LintErrorKind::MismatchArguments {
                                    command: name,
                                    expected,
                                    found,
                                },
                            });
                        }
                    }
                    match config {
                        CommandConfig::Env(EnvConfig {
                            parent_requirement,
                            provides,
                            ..
                        })
                        | CommandConfig::Wrap(WrapConfig {
                            parent_requirement,
                            provides,
                            ..
                        })
                        | CommandConfig::Template(TemplateConfig {
                            parent_requirement,
                            provides,
                            ..
                        })
                        | CommandConfig::Regex(RegexConfig {
                            parent_requirement,
                            provides,
                            ..
                        }) => {
                            check_parent_requirement(
                                parent_requirement,
                                &provided_environments,
                                line_num,
                                leading_chars as usize,
                                &name,
                            )?;
                            if let Some(p) = provides {
                                provided_environments.push(p.clone());
                            }
                        }
                    }

                    for child in children {
                        check_tree(
                            child,
                            Some(indent_unit),
                            configs,
                            provided_environments.clone(),
                        )?;
                    }
                }
                None => {
                    unreachable!("登録済みのみコマンドに変換されるためここには来ないはず");
                    // return Err(LintError {
                    //     line: line_num,
                    //     character: indent as usize * indent_unit,
                    //     kind: crate::errors::LintErrorKind::UnknownCommand(name),
                    // });
                }
            };
        }
        _ => (),
    }
    Ok(())
}

fn check_parent_requirement(
    parent_requirement: &Option<String>,
    provided_environments: &[String],
    line_num: usize,
    leading_chars: usize,
    name: &str,
) -> Result<(), LintError> {
    if let Some(r) = parent_requirement
        && !provided_environments.contains(r)
    {
        return Err(LintError {
            line: line_num,
            character: leading_chars,
            kind: LintErrorKind::RequiredEnvNotFound {
                command: name.to_string(),
                required: r.clone(),
            },
        });
    }
    Ok(())
}
