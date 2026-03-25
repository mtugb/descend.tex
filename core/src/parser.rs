use std::{cmp::Ordering, collections::HashMap};

use regex::Regex;

use crate::{
    errors::{ParseError, ParseErrorKind},
    models::{config::CommandConfig, node::Node},
};

pub fn parse_to_tree(
    sample: &str,
    configs: &HashMap<String, CommandConfig>,
) -> Result<Node, ParseError> {
    let mut stack: Vec<(Node, usize)> = vec![(
        Node::Root {
            children: Vec::new(),
            line_num: 0,
        },
        0,
    )];
    for (i, line) in sample.lines().enumerate() {
        if is_empty_line(line) {
            continue;
        }
        let trimed = line.trim().to_string();
        let last_indent: usize = stack.last().unwrap().1;
        let current_indent = get_indent(line);
        let indent_comparison = current_indent.cmp(&last_indent);
        match indent_comparison {
            Ordering::Greater | Ordering::Equal => {}
            Ordering::Less => {
                fold_stack(&mut stack, current_indent)?;
            }
        }

        let mut is_command = false;

        for (key, config) in configs {
            let regex_pattern: &Regex = match config {
                CommandConfig::Template(t) => &t.pattern,
                CommandConfig::Env(e) => &e.pattern,
                CommandConfig::Regex(r) => &r.pattern,
                CommandConfig::Wrap(w) => &w.pattern,
            };
            let captures = regex_pattern.captures(&trimed);
            match captures {
                Some(c) => {
                    // このコマンドパターンにマッチした
                    let captures = c
                        .iter()
                        .skip(1)
                        .map(|m| m.map(|m| m.as_str().to_string()))
                        .collect::<Option<Vec<String>>>();
                    match captures {
                        Some(c) => {
                            // すべてのキャプチャグループの値が省略されずに存在している。
                            // 空文字にマッチした場合も含む。
                            is_command = true;
                            stack.push((
                                Node::command(trimed.clone(), key.clone(), Some(c), i),
                                current_indent,
                            ));
                            break;
                        }
                        None => {
                            // パターンには省略可能なキャプチャグループが少なくとも１つ存在し、
                            // 実際に省略された。

                            return Err(ParseError {
                                line: 404,
                                col: 100,
                                kind: ParseErrorKind::DangerousCaptureGroups {
                                    field_name: key.clone(),
                                },
                            });
                        }
                    }
                }
                None => {
                    // このコマンドパターンにマッチしなかった
                    continue;
                }
            }
        }

        if !is_command {
            stack.push((
                Node::Leaf {
                    content: trimed,
                    line_num: i,
                },
                current_indent,
            ));
        }
    }

    fold_stack(&mut stack, 0)?;

    let root = stack.first().unwrap();
    Ok(root.clone().0)
}

fn fold_stack(stack: &mut Vec<(Node, usize)>, into: usize) -> Result<(), ParseError> {
    let mut wait: Vec<(Node, usize)> = Vec::new();

    while stack
        .last()
        .ok_or(ParseError {
            line: 0,
            col: 0,
            kind: ParseErrorKind::EmptyStackForFoldStack,
        })?
        .1
        > into
    {
        // popped がstack自体を奪うわけではないので引き続きstackは参照可能。
        let popped = stack.pop().unwrap();
        // usizeはCopyトレイトを持つので、今後もpoppedへのアクセスは可能
        let popped_indent = popped.1;
        // stackの最後の要素の可変参照を得る。「以降Stackの直接参照は不可能」
        let top = stack.last_mut().unwrap();
        // usizeはCopyトレイトを持つので、今後もtopへのアクセスは可能
        let top_indent = top.1;
        // waitに中身を完全に渡したのでここ以下でpoppedの参照は不可.
        wait.push(popped);

        if popped_indent != top_indent {
            // &mutにすることで、参照なので無駄なメモリ消費が無く、なおかつmutなのでchildrenの変更が可能
            match &mut top.0 {
                Node::Root { children, .. } | Node::Command { children, .. } => {
                    while !wait.is_empty() {
                        children.push(wait.pop().unwrap().0);
                    }
                }
                Node::Leaf { content, line_num } => {
                    return Err(ParseError {
                        line: *line_num,
                        col: 0,
                        kind: ParseErrorKind::LeafHavingChildren(content.clone()),
                    });
                }
            }
        }
    }
    Ok(())
}

fn get_indent(text: &str) -> usize {
    let mut i = 0;
    for c in text.chars() {
        if !c.is_ascii_whitespace() {
            break;
        }
        i += 1;
    }
    i
}

fn is_empty_line(line: &str) -> bool {
    line.is_empty() || line.chars().filter(|c| !c.is_ascii_whitespace()).count() == 0
}
