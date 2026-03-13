use std::{
    cmp::Ordering,
    collections::HashMap,
    fmt::{self},
    fs,
    path::Path,
};

use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;

#[derive(Clone)]
pub enum Node {
    /// ルートノード（ドキュメント全体）
    Root(Vec<Node>),

    /// 特定のコマンド（mat, sumなど）
    Command {
        name: String,
        children: Vec<Node>, // 子要素もNodeなので再帰的
    },

    /// 最小単位（x + y など、これ以上分解しない文字列）
    Leaf(String),
}
impl Node {
    fn command(name: String) -> Node {
        Node::Command {
            name,
            children: Vec::new(),
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
            Node::Root(children) => {
                writeln!(f, "{}Root", indent)?;
                for child in children {
                    child.fmt_tree(f, depth + 1)?;
                }
            }
            Node::Command { name, children } => {
                writeln!(f, "{}Command({})", indent, name)?;
                for child in children {
                    child.fmt_tree(f, depth + 1)?;
                }
            }
            Node::Leaf(s) => {
                writeln!(f, "{}Leaf({})", indent, s)?;
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
    pub env_name: String,
    pub alias: Option<Vec<String>>,
}

pub fn parse_to_tree(sample: &str, configs: &HashMap<String, CommandConfig>) -> Result<Node> {
    let mut stack: Vec<(Node, usize)> = vec![(Node::Root(Vec::new()), 0)];
    for line in sample.lines() {
        if is_empty_line(line) {
            continue;
        }
        let last_indent: usize = stack.last().unwrap().1;
        let current_indent = get_indent(line);
        let indent_comparison = current_indent.cmp(&last_indent);
        match indent_comparison {
            Ordering::Greater | Ordering::Equal => {
                let trimed = line.trim().to_string();
                if configs.contains_key(&trimed.to_string()) {
                    if indent_comparison == Ordering::Equal {
                        let (finished_node, _) = stack.pop().unwrap();
                        if let Some((Node::Root(children) | Node::Command { children, .. }, _)) =
                            stack.last_mut()
                        {
                            children.push(finished_node);
                        }
                    }
                    stack.push((Node::command(trimed), current_indent));
                } else {
                    //this condition is always true
                    if let Some((Node::Root(children) | Node::Command { children, .. }, _)) =
                        stack.last_mut()
                    {
                        children.push(Node::Leaf(trimed));
                    }
                }
            }
            Ordering::Less => {
                while let Some(top) = stack.last() {
                    if top.1 > current_indent {
                        let (finished_node, _) = stack.pop().unwrap();
                        if let Some((Node::Root(children) | Node::Command { children, .. }, _)) =
                            stack.last_mut()
                        {
                            children.push(finished_node);
                        }
                    } else {
                        break;
                    }
                }
            }
        }
    }

    while stack.len() > 1 {
        let (finished_node, _) = stack.pop().unwrap();
        if let Some((Node::Root(children) | Node::Command { children, .. }, _)) = stack.last_mut() {
            children.push(finished_node);
        }
    }

    let root = stack.first().unwrap();
    Ok(root.clone().0)
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
            Node::Root(children) => {
                let parts: Result<Vec<String>> = children
                    .iter()
                    .map(|c| self.compile_command_into_latex(c))
                    .collect();
                Ok(parts?.join(""))
            }
            Node::Command { name, children } => match self.configs.get(name) {
                Some(config) => match config {
                    CommandConfig::Template(t) => self.format_template(name, children, t),
                    CommandConfig::Env(c) => Ok(self.format_environment(&c.env_name, children)?),
                    CommandConfig::Regex(_) => {
                        todo!()
                    }
                },
                None => Err(anyhow::anyhow!("no command found")),
            },
            Node::Leaf(text) => Ok(text.to_string()),
        }
    }
    fn format_environment(&self, name: &str, children: &[Node]) -> Result<String> {
        let mut command = String::new();
        command.push_str("\\begin{");
        command.push_str(name);
        command.push('}');
        command.push('\n');
        let body = children
            .iter()
            .map(|child| self.compile_command_into_latex(child))
            .collect::<Result<Vec<_>>>()?
            .join(" \\\\ \n");
        command.push_str(&body);
        command.push('\n');
        command.push_str("\\end{");
        command.push_str(name);
        command.push('}');
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
        if let CommandConfig::Template(t) = &config {
            if t.alias.is_none() {
                continue;
            }
            t.alias.as_ref().unwrap().iter().for_each(|a| {
                map_extended.insert(a.clone(), config.clone());
            });
        }
        map_extended.insert(name, config);
    }
    Ok(map_extended)
}
