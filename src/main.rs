use std::{
    cmp::Ordering,
    collections::HashMap,
    fmt::{self},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::Deserialize;

enum Node {
    /// ルートノード（ドキュメント全体）
    Root(Vec<Node>),

    /// 特定のコマンド（mat, sumなど）
    Command {
        name: &'static str,
        children: Vec<Node>, // 子要素もNodeなので再帰的
    },

    /// 最小単位（x + y など、これ以上分解しない文字列）
    Leaf(&'static str),
}
impl Node {
    fn command(name: &'static str) -> Node {
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

#[derive(Debug, Deserialize, Clone)]
enum RenderType {
    Template,
    Environment,
}

#[derive(Debug, Deserialize, Clone)]
struct CommandConfig {
    #[serde(rename = "type")]
    render_type: RenderType,
    template: Option<String>,
    env_name: Option<String>,
    alias: Option<Vec<String>>,
}

fn main() -> Result<()> {
    let sample = r"
frac
  lim
    n
    inf
    n^2
  frac
    1
    n
    ";

    // TODO: 保存先をOS依存なしに
    let config = load_command_config(&PathBuf::from("./commands.toml"))?;
    let known_commands = config.iter().map(|(key, _)| key).collect::<Vec<_>>();

    let mut stack: Vec<(Node, i32)> = vec![(Node::Root(Vec::new()), -1)];
    for line in sample.lines() {
        if is_empty_line(line) {
            continue;
        }
        let last_indent: i32 = stack.last().unwrap().1;
        let current_indent = get_indent(line);
        let indent_comparison = current_indent.cmp(&last_indent);
        match indent_comparison {
            Ordering::Greater | Ordering::Equal => {
                let trimed = line.trim();
                if known_commands.contains(&&trimed.to_string()) {
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

    println!("{:?}", stack.first().unwrap().0);
    println!("{}", get_latex(&stack.first().unwrap().0, &config));
    Ok(())
}

fn get_indent(text: &str) -> i32 {
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

fn get_latex(node: &Node, configs: &HashMap<&str, CommandConfig>) -> Result<String> {
    match node {
        Node::Root(children) => Ok(children
            .iter()
            .map(|c| get_latex(c, configs)?)
            .collect::<Vec<_>>()
            .join("")),
        Node::Command { name, children } => match configs.get(name) {
            Some(config) => match config.render_type {
                RenderType::Template => format_template(name, children, configs),
                RenderType::Environment => {
                    // env_name があれば使い、なければ name を使う
                    let env_name = config.env_name.as_deref().unwrap_or(name);
                    Ok(format_environment(env_name, children, configs)?)
                }
            },
            None => Err(anyhow!("no command found")),
        },
        Node::Leaf(text) => Ok(text.to_string()),
    }
}

// fn format_command_a(
//     name: &str,
//     children: &Vec<Node>,
//     configs: &HashMap<&str, RenderType>,
// ) -> String {
//     let mut command = String::new();
//     command.push('\\');
//     command.push_str(name);
//     for child in children {
//         command.push('{');
//         command.push_str(&get_latex(child, configs));
//         command.push('}');
//     }
//     command
// }

fn format_environment(
    name: &str,
    children: &[Node],
    configs: &HashMap<&str, CommandConfig>,
) -> Result<String> {
    let mut command = String::new();
    command.push_str("\\begin{");
    command.push_str(name);
    command.push('}');
    command.push('\n');
    let body = children
        .iter()
        .map(|child| get_latex(child, configs)?)
        .collect::<Vec<_>>()
        .join(" \\\\ \n");
    command.push_str(&body);
    command.push('\n');
    command.push_str("\\end{");
    command.push_str(name);
    command.push('}');
    Ok(command)
}

fn format_template(
    name: &str,
    children: &[Node],
    configs: &HashMap<&str, CommandConfig>,
) -> Result<String> {
    let config = configs.get(name).unwrap(); // get_latexで存在確認済み
    let mut template = config
        .template
        .as_ref() // これないと怒られる。よくわかってない
        .with_context(|| {
            format!(
                "Command {} is Template type but has no template string",
                name
            )
        })?
        .clone();

    for (i, child) in children.iter().enumerate() {
        let placeholder = format!("{{{}}}", i);
        let replacement = get_latex(child, configs)?;
        template = template.replace(&placeholder, &replacement);
    }

    Ok(template)
}

fn load_command_config(path: &Path) -> Result<HashMap<String, CommandConfig>> {
    // TODO: 重複時に警告するプロセスを作成
    let content = fs::read_to_string(path)?;
    let map: HashMap<String, CommandConfig> = toml::from_str(&content)?;
    let mut map_extended: HashMap<String, CommandConfig> = HashMap::new();
    for (name, config) in map {
        if let Some(aliases) = &config.alias {
            aliases.iter().for_each(|a| {
                map_extended.insert(a.clone(), config.clone());
            });
        }
        map_extended.insert(name, config);
    }
    Ok(map_extended)
}
