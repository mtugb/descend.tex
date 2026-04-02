use std::fmt;

// Node
#[derive(Clone)]
pub enum Node {
    /// ルートノード（ドキュメント全体）
    Root {
        children: Vec<Node>,
        line_num: usize,
        indent: i32,
    },

    /// 特定のコマンド（mat, sumなど）
    Command {
        name: String,
        config_key: String,
        captures: Option<Vec<String>>,
        children: Vec<Node>, // 子要素もNodeなので再帰的
        line_num: usize,
        indent: i32,
    },

    /// 最小単位（x + y など、これ以上分解しない文字列）
    Leaf {
        content: String,
        line_num: usize,
        indent: i32,
    },
}
impl Node {
    pub fn command(
        name: String,
        config_key: String,
        captures: Option<Vec<String>>,
        line_num: usize,
        indent: i32,
    ) -> Node {
        Node::Command {
            name,
            config_key,
            captures,
            children: Vec::new(),
            line_num,
            indent,
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
