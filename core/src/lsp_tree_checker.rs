use std::collections::HashMap;

use crate::{
    errors::LintError,
    models::{config::CommandConfig, node::Node},
};

// This function is for LSP
pub fn check_tree(
    current: Node,
    indent_unit: Option<usize>,
    configs: &HashMap<String, CommandConfig>,
) -> Result<(), LintError> {
    let indent_unit = indent_unit.unwrap_or(4);
    match current {
        Node::Root { children, .. } => {
            for child in children {
                check_tree(child, Some(indent_unit), configs)?;
            }
            return Ok(());
        }
        Node::Command {
            name,
            config_key,
            captures: _,
            children,
            line_num,
            indent,
        } => {
            match configs.get(&config_key) {
                Some(config) => {
                    // コマンドだった
                    if let CommandConfig::Template(t) = config {
                        let expected = t.args_count;
                        let found = children.len();
                        if expected != found {
                            return Err(LintError {
                                line: line_num,
                                character: indent as usize * indent_unit,
                                kind: crate::errors::LintErrorKind::MismatchArguments {
                                    command: name,
                                    expected,
                                    found,
                                },
                            });
                        }
                    }
                    for child in children {
                        check_tree(child, Some(indent_unit), configs)?;
                    }
                }
                None => {
                    unreachable!("登録済みのみコマンドに変換されるためここには来ないはず");
                    return Err(LintError {
                        line: line_num,
                        character: indent as usize * indent_unit,
                        kind: crate::errors::LintErrorKind::UnknownCommand(name),
                    });
                }
            };
        }
        _ => (),
    }
    Ok(())
}
