use mytex::{
    config::{load_command_config, load_replacements_config},
    parser::parse_to_tree,
    renderer::TreeLatexConverter,
};

fn main() -> anyhow::Result<()> {
    let command_configs = load_command_config(None)?;
    let replacements_config = load_replacements_config(None)?;
    // dbg!(&configs);

    let root = parse_to_tree(
        r"
        $
         |mat|
          a b
          c d
         = a*d - b*c
        ",
        &command_configs,
        None,
    )?;

    let converter = TreeLatexConverter::new(&command_configs, replacements_config)?;

    let latex = converter.compile_tree_into_latex(&root)?;

    println!("{:?}", root);
    println!("{}", latex);
    Ok(())
}
