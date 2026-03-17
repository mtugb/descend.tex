use std::{
    cmp::Ordering,
    collections::HashMap,
    fmt::{self, format},
    fs,
    path::Path,
};

use anyhow::{Context, Result, bail, ensure};
use regex::Regex;
use serde::Deserialize;

#[derive(Clone)]
pub enum Node {
    /// ルートノード（ドキュメント全体）
    Root {
        children: Vec<Node>,
        line_num: usize,
    },

    /// 特定のコマンド（mat, sumなど）
    Command {
        name: String,
        config_key: String,
        captures: Option<Vec<String>>,
        children: Vec<Node>, // 子要素もNodeなので再帰的
        line_num: usize,
    },

    /// 最小単位（x + y など、これ以上分解しない文字列）
    Leaf { content: String, line_num: usize },
}
impl Node {
    fn command(
        name: String,
        config_key: String,
        captures: Option<Vec<String>>,
        line_num: usize,
    ) -> Node {
        Node::Command {
            name,
            config_key,
            captures,
            children: Vec::new(),
            line_num,
        }
    }
}
impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_tree(f, 0)
    }
}
impl Node {
    fn fmt_tree(&self, f: &mut fmt::Formatter<'_>, depth: usize) -> fmt::Result {
        let indent = "  ".repeat(depth);
        match self {
            Node::Root { children, .. } => {
                writeln!(f, "{}Root", indent)?;
                for child in children {
                    child.fmt_tree(f, depth + 1)?;
                }
            }
            Node::Command { name, children, .. } => {
                writeln!(f, "{}Command({})", indent, name)?;
                for child in children {
                    child.fmt_tree(f, depth + 1)?;
                }
            }
            Node::Leaf { content, .. } => {
                writeln!(f, "{}Leaf({})", indent, content)?;
            }
        }
        Ok(())
    }
}

// #[derive(Debug, Deserialize, Clone)]
// pub enum RenderType {
//     Template,
//     Environment,
// }

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type")] // "type" フィールドの値で分岐
pub enum CommandConfig {
    #[serde(rename = "Template")]
    Template(TemplateConfig),

    #[serde(rename = "Regex")]
    Regex(RegexConfig),

    #[serde(rename = "Environment")]
    Env(EnvConfig),
}

#[derive(Debug, Deserialize, Clone)]
pub struct TemplateConfig {
    pub pattern: String,
    pub template: String,
    pub args_count: usize,
    pub alias: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RegexConfig {
    pub pattern: String,
    pub template: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct EnvConfig {
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
}

#[derive(Debug, Deserialize, Clone)]
pub struct Replacement {
    pub from: String,
    pub to: String,
}

pub fn parse_to_tree(sample: &str, configs: &HashMap<String, CommandConfig>) -> Result<Node> {
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
                fold_stack(&mut stack, current_indent).with_context(|| {
                    format!(
                        "Failed to parse block starting at line {}: \"{}\"",
                        i + 1,
                        trimed
                    )
                })?;
            }
        }

        let mut is_command = false;

        for (key, config) in configs {
            let raw_pattern: &str = match config {
                CommandConfig::Template(t) => &t.pattern,
                CommandConfig::Env(e) => &e.pattern,
                CommandConfig::Regex(r) => &r.pattern,
            };
            let sandwiched_pattern: String = if raw_pattern.starts_with('^') {
                raw_pattern.to_string()
            } else {
                format!("^{}$", raw_pattern)
            };
            let regex_pattern = Regex::new(&sandwiched_pattern).with_context(|| {
                format!(
                    "invalid regex pattern detected in command config: field \"{}\" in {} ",
                    raw_pattern, key
                )
            })?;
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
                            bail!("省略可能なキャプチャグループは使用できません");
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

    fold_stack(&mut stack, 0).with_context(|| "Failed to fold stacks")?;

    let root = stack.first().unwrap();
    Ok(root.clone().0)
}

fn fold_stack(stack: &mut Vec<(Node, usize)>, into: usize) -> Result<()> {
    let mut wait: Vec<(Node, usize)> = Vec::new();

    while stack
        .last()
        .context("fold_stack requires stack with any content")?
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
                    bail!(
                        "'{}' (at line {}) is not a command and cannot have children",
                        content,
                        *line_num + 1
                    )
                }
            }
        }
    }
    Ok(())
}

fn format_my_error(body: &str, line_num: usize, raw_line: &str) -> String {
    format!(
        "Error: {}\n  at line {}: \"{}\"",
        body,
        line_num + 1, // プログラム上の0開始を、人間用の1開始に変換
        raw_line.trim()
    )
}

fn trigger_my_error(body: &str, line_num: usize, raw_line: &str) -> Result<()> {
    bail!(format_my_error(body, line_num, raw_line));
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

pub struct CommandLatexConverter<'a> {
    pub configs: &'a HashMap<String, CommandConfig>,
}

impl<'a> CommandLatexConverter<'a> {
    pub fn compile_command_into_latex(&self, node: &Node) -> Result<String> {
        match node {
            Node::Root { children, .. } => {
                let parts: Result<Vec<String>> = children
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
                },
                None => bail!("{} is unknown command type", name),
            },
            Node::Leaf { content: text, .. } => Ok(text.to_string()),
        }
    }
    fn format_environment(&self, config: &EnvConfig, children: &[Node]) -> Result<String> {
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
            .collect::<Result<Vec<_>>>()?
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

    fn format_template(
        &self,
        name: &str,
        children: &[Node],
        config: &TemplateConfig,
    ) -> Result<String> {
        let mut template = config.template.clone();

        let required = config.args_count;
        ensure!(
            children.len() == required,
            "コマンド '{}' は引数を {} 個必要としますが、{} 個しかありません。",
            name,
            required,
            children.len()
        );
        for (i, child) in children.iter().enumerate() {
            // $0, $1, $2... を探して置換
            let placeholder = format!("${}", i);
            let replacement = self.compile_command_into_latex(child)?;
            template = template.replace(&placeholder, &replacement);
        }

        Ok(template)
    }

    fn format_regex(&self, captures: Option<Vec<String>>, config: &RegexConfig) -> Result<String> {
        let mut template = config.template.clone();
        let captures = captures.unwrap_or_default();
        let placeholder = Regex::new(r"\$[0-9]+").unwrap();
        let placeholder_count = placeholder.find_iter(&template).count();
        ensure!(
            captures.len() == placeholder_count,
            "テンプレート '{}' は引数を {} 個必要としますが、有効なキャプチャーグループが{} 個しかありません。",
            template,
            placeholder_count,
            captures.len()
        );
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

const DEFAULT_CONFIG_STR: &str = include_str!("../commands.toml");
pub fn load_command_config(path: Option<&Path>) -> Result<HashMap<String, CommandConfig>> {
    // TODO: 重複時に警告するプロセスを作成
    let content = match path {
        Some(p) => fs::read_to_string(p)?,
        None => DEFAULT_CONFIG_STR.to_string(),
    };
    let map: HashMap<String, CommandConfig> = toml::from_str(&content)?;
    let mut map_extended: HashMap<String, CommandConfig> = HashMap::new();
    for (name, config) in map {
        let aliases: Option<&Vec<String>> = match &config {
            CommandConfig::Template(t) => t.alias.as_ref(),
            CommandConfig::Env(e) => e.alias.as_ref(),
            CommandConfig::Regex(_) => None,
        };
        if let Some(aliases) = aliases {
            for alias in aliases {
                let aliased_config = match &config {
                    CommandConfig::Template(t) => CommandConfig::Template(TemplateConfig {
                        pattern: alias.clone(),
                        ..t.clone()
                    }),
                    CommandConfig::Env(e) => CommandConfig::Env(EnvConfig {
                        pattern: alias.clone(),
                        ..e.clone()
                    }),
                    CommandConfig::Regex(_) => unreachable!(),
                };
                map_extended.insert(alias.clone(), aliased_config);
            }
        }
        map_extended.insert(name, config);
    }
    Ok(map_extended)
}
