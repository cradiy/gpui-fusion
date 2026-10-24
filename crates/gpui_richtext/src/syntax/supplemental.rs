use syntect::parsing::{Scope, SyntaxDefinition, SyntaxSetBuilder};

pub(super) fn register(builder: &mut SyntaxSetBuilder) {
    if let Some(mut jsx) = builder
        .syntaxes()
        .iter()
        .find(|syntax| syntax.name == "TypeScriptReact")
        .cloned()
    {
        jsx.name = "JavaScriptReact".into();
        jsx.file_extensions = vec!["jsx".into()];
        jsx.scope = Scope::new("source.jsx").unwrap();
        builder.add(jsx);
    }
    for source in [
        include_str!("powershell.sublime-syntax"),
        include_str!("visual-basic.sublime-syntax"),
        include_str!("json-extended.sublime-syntax"),
    ] {
        builder.add(
            SyntaxDefinition::load_from_str(source, true, None).expect("valid embedded syntax"),
        );
    }
}
