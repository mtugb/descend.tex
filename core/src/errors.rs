#[derive(thiserror::Error, Debug)]
#[error("Error at line {line}, character {character}: {kind}")]
pub struct ParseError {
    pub line: usize,
    pub character: usize,
    pub kind: ParseErrorKind, // さっき定義した Enum をここに入れる
}

#[derive(thiserror::Error, Debug)]
pub enum ParseErrorKind {
    #[error("Indentation mismatch: expected {expected}, found {found}")]
    Indent { expected: usize, found: usize },

    #[error("Invalid indent width: {found} is not a multiple of {indent_unit}")]
    InvalidIndentWidth { found: usize, indent_unit: usize },

    #[error("Internal stack error: {0}")]
    Stack(String), // fold_stackからのエラーメッセージなどを入れる

    #[error("Unknown command: {0}")]
    UnknownCommand(String),

    #[error(
        "Illegal capture group: omittable capture group cannot be used: pattern for field \"{field_name}\""
    )]
    DangerousCaptureGroups { field_name: String },

    #[error("Empty stack was given to fold_stack function")]
    EmptyStackForFoldStack,

    #[error("'{0}' is not a command and cannot have children")]
    LeafHavingChildren(String),
}

#[derive(thiserror::Error, Debug)]
pub enum ConfigError {
    #[error("Failed to parse TOML file: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("Unknown field: {0}")]
    UnknownField(String),
    #[error("Config file \"{0}\" doesn't exist")]
    CommandConfigFileNotFound(String),
    #[error("Replacements file \"{0}\" not found")]
    ReplacementsFileNotFound(String),
    #[error("Invalid value for field \"{field_name}\": {reason}")]
    Value { field_name: String, reason: String },
    #[error("Invalid regex in field \"{field_name}\": {source}")]
    Regex {
        field_name: String,
        source: regex::Error,
    },
}

// のちに立体的なエラーにする。コマンドの種類で分けてsource引用する
#[derive(thiserror::Error, Debug)]
pub enum RenderError {
    #[error("Arguments for \"{command}\" count mismatch: expected {expected}, found {found}")]
    MismatchArguments {
        command: String,
        expected: usize,
        found: usize,
    },
    #[error(
        "CaptureGroups for Template(\"{template}\") count mismatch: expected {expected}, found {found}"
    )]
    MismatchTemplate {
        template: String,
        expected: usize,
        found: usize,
    },
    #[error("Failed to parse TOML file for leaf replacements: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("Invalid regex for leaf replacements: {source}")]
    Regex { source: regex::Error },
    #[error("Unknown command type: {0}")]
    UnknownCommand(String),
}

pub struct LintError {
    pub line: usize,
    pub character: usize,
    pub kind: LintErrorKind,
}

#[derive(thiserror::Error, Debug)]
pub enum LintErrorKind {
    #[error("Arguments for \"{command}\" count mismatch: expected {expected}, found {found}")]
    MismatchArguments {
        command: String,
        expected: usize,
        found: usize,
    },
    #[error("Unknown command: {0}")]
    UnknownCommand(String),
    // 他に必要なものがあれば
}
