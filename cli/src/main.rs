use mytex::load_command_config;

fn main() -> anyhow::Result<()> {
    let configs = load_command_config(None)?;
    // dbg!(&configs);
    let converter = mytex::CommandLatexConverter { configs: &configs };

    let root = mytex::parse_to_tree(
        r"
        # hello
        this is a plain text
        # world
        $$
         frac
          1
          2
         mat
          1 2
          3 4
        ",
        &configs,
    )?;

    let latex = converter.compile_command_into_latex(&root)?;

    println!("{:?}", root);
    println!("{}", latex);
    Ok(())
}
