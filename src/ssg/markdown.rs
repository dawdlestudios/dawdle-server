use comrak::{
    plugins::syntect::SyntectAdapterBuilder, ExtensionOptionsBuilder, ParseOptionsBuilder, Plugins,
    RenderOptionsBuilder,
};

pub fn md_to_html(buf: &str) -> String {
    let options = comrak::ComrakOptions {
        parse: ParseOptionsBuilder::default().build().unwrap(),
        render: RenderOptionsBuilder::default()
            .unsafe_(true)
            .build()
            .unwrap(),
        extension: ExtensionOptionsBuilder::default()
            .front_matter_delimiter(Some("---".to_string()))
            .build()
            .unwrap(),
    };

    let mut plugins = Plugins::default();
    let syntax_highlighter = SyntectAdapterBuilder::default()
        .theme("base16-ocean.dark")
        .build();
    plugins.render.codefence_syntax_highlighter = Some(&syntax_highlighter);

    comrak::markdown_to_html_with_plugins(&buf, &options, &plugins)
}
