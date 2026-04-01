use std::{
    fs,
    path::{Path, PathBuf},
};

use clap::Parser;
use mytex::{config::load_command_config, parser::parse_to_tree, renderer::CommandLatexConverter};

/// Compile human-friendly-tex into latex
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// input file
    input: PathBuf,
    /// output file
    #[arg(short, long)]
    output: Option<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let Args {
        input: input_file_path,
        output: output_file_path,
    } = Args::parse();
    validate_input_file_path(&input_file_path)?;
    let input_str = fs::read_to_string(&input_file_path)?;
    let resolved_output_file_path =
        output_file_path.unwrap_or_else(|| input_file_path.with_extension("tex"));
    let output_str = compile_htex_into_latex(&input_str)?;
    fs::write(resolved_output_file_path, output_str)?;
    Ok(())
}

fn validate_input_file_path(path: &Path) -> anyhow::Result<()> {
    anyhow::ensure!(
        path.extension() == Some("htex".as_ref()),
        "Input path extension must be \".htex\", but found {}",
        path.as_os_str().to_str().unwrap()
    );
    Ok(())
}

fn compile_htex_into_latex(input_str: &str) -> anyhow::Result<String> {
    let configs = load_command_config(None)?;
    let converter = CommandLatexConverter { configs: &configs };
    let root = parse_to_tree(input_str, &configs)?;
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
        converter.compile_command_into_latex(&root)?
    );
    Ok(latex)
}
