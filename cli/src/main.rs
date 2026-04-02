use std::{
    fs,
    path::{Path, PathBuf},
};

use clap::Parser;
use mytex::{
    config::{load_command_config, load_replacements_config},
    parser::parse_to_tree,
    renderer::TreeLatexConverter,
};

/// Compile human-friendly-tex into latex
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// input file
    input: PathBuf,
    /// output file
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// path to commands config file (.toml)
    #[arg(short, long)]
    commands: Option<PathBuf>,
    /// path to replacements config file (.toml)
    #[arg(short, long)]
    replacements: Option<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let Args {
        input: input_file_path,
        output: output_file_path,
        commands: commands_config_path,
        replacements: replacements_config_path,
    } = Args::parse();
    validate_input_file_path(&input_file_path)?;
    let input_str = fs::read_to_string(&input_file_path)?;
    let resolved_output_file_path =
        output_file_path.unwrap_or_else(|| input_file_path.with_extension("tex"));
    let output_str =
        compile_dtex_into_latex(&input_str, commands_config_path, replacements_config_path)?;
    fs::write(resolved_output_file_path, output_str)?;
    Ok(())
}

fn validate_input_file_path(path: &Path) -> anyhow::Result<()> {
    anyhow::ensure!(
        path.extension() == Some("dtex".as_ref()),
        "Input path extension must be \".dtex\", but found {}",
        path.as_os_str().to_str().unwrap()
    );
    Ok(())
}

fn compile_dtex_into_latex(
    input_str: &str,
    commands_config_path: Option<PathBuf>,
    replacements_config_path: Option<PathBuf>,
) -> anyhow::Result<String> {
    // Option<PathBuf> ->
    // Option<&Path>の変換をおこなう(&PathBufは本来≠&Pathであることに注意
    // Path = PathBufのスライス。所有権は持たない)
    let command_configs = load_command_config(commands_config_path.as_deref())?;
    let replacements_config = load_replacements_config(replacements_config_path.as_deref())?;
    let converter = TreeLatexConverter::new(&command_configs, replacements_config)?;
    let root = parse_to_tree(input_str, &command_configs, None)?;
    // {{, }} は{, }のformatでのエスケープ
    let latex = format!(
        r"
        \documentclass{{article}}
        \usepackage{{graphicx}}
        \usepackage{{amsmath}}
        \begin{{document}}
        {}
        \end{{document}}
    ",
        converter.compile_tree_into_latex(&root)?
    );
    Ok(latex)
}
