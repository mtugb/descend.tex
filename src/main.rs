use std::{cmp::Ordering, fmt};

fn main() {
    let known_commands = ["mat", "lim", "frac"];
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
    let sample = r"
mat
  1/2
  frac
    x
    y
    ";
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
                if known_commands.contains(&trimed) {
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
