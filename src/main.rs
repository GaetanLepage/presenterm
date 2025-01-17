use clap::{error::ErrorKind, CommandFactory, Parser};
use comrak::Arena;
use presenterm::{
    CommandSource, Config, Exporter, HighlightThemeSet, LoadThemeError, MarkdownParser, PresentMode,
    PresentationThemeSet, Presenter, Resources, Themes, TypstRender,
};
use std::{
    env,
    path::{Path, PathBuf},
};

/// Run slideshows from your terminal.
#[derive(Parser)]
#[command()]
#[command(author, version, about = create_splash(), long_about = create_splash(), arg_required_else_help = true)]
struct Cli {
    /// The path to the markdown file that contains the presentation.
    #[clap(group = "target")]
    path: Option<PathBuf>,

    /// Export the presentation as a PDF rather than displaying it.
    #[clap(short, long)]
    export_pdf: bool,

    /// Generate the PDF metadata without generating the PDF itself.
    #[clap(long, hide = true)]
    generate_pdf_metadata: bool,

    /// Run in export mode.
    #[clap(long, hide = true)]
    export: bool,

    /// Whether to use presentation mode.
    #[clap(short, long, default_value_t = false)]
    present: bool,

    /// The theme to use.
    #[clap(short, long, default_value = "dark")]
    theme: String,

    /// Display acknowledgements.
    #[clap(long, group = "target")]
    acknowledgements: bool,
}

fn create_splash() -> String {
    let crate_version = env!("CARGO_PKG_VERSION");

    format!(
        r#"
  ┌─┐┬─┐┌─┐┌─┐┌─┐┌┐┌┌┬┐┌─┐┬─┐┌┬┐
  ├─┘├┬┘├┤ └─┐├┤ │││ │ ├┤ ├┬┘│││
  ┴  ┴└─└─┘└─┘└─┘┘└┘ ┴ └─┘┴└─┴ ┴ v{}
    A terminal slideshow tool 
                    @mfontanini/presenterm
"#,
        crate_version,
    )
}

fn load_customizations() -> Result<(Config, Themes), Box<dyn std::error::Error>> {
    let Ok(home_path) = env::var("HOME") else {
        return Ok(Default::default());
    };
    let home_path = PathBuf::from(home_path);
    let config_path = home_path.join(".config/presenterm");
    let themes = load_themes(&config_path)?;
    let config = Config::load(&config_path.join("config.yaml"))?;
    Ok((config, themes))
}

fn load_themes(config_path: &Path) -> Result<Themes, Box<dyn std::error::Error>> {
    let themes_path = config_path.join("themes");

    let mut highlight_themes = HighlightThemeSet::default();
    highlight_themes.register_from_directory(&themes_path.join("highlighting"))?;

    let mut presentation_themes = PresentationThemeSet::default();
    let register_result = presentation_themes.register_from_directory(&themes_path);
    if let Err(e @ (LoadThemeError::Duplicate(_) | LoadThemeError::Corrupted(..))) = register_result {
        return Err(e.into());
    }

    let themes = Themes { presentation: presentation_themes, highlight: highlight_themes };
    Ok(themes)
}

fn display_acknowledgements() {
    let acknowledgements = include_bytes!("../bat/acknowledgements.txt");
    println!("{}", String::from_utf8_lossy(acknowledgements));
}

fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    let (config, themes) = load_customizations()?;

    let default_theme_name = config.defaults.theme.unwrap_or(cli.theme);
    let Some(default_theme) = themes.presentation.load_by_name(&default_theme_name) else {
        let mut cmd = Cli::command();
        let valid_themes = themes.presentation.theme_names().join(", ");
        let error_message = format!("invalid theme name, valid themes are: {valid_themes}");
        cmd.error(ErrorKind::InvalidValue, error_message).exit();
    };

    let mode = match (cli.present, cli.export) {
        (true, _) => PresentMode::Presentation,
        (false, true) => PresentMode::Export,
        (false, false) => PresentMode::Development,
    };
    let arena = Arena::new();
    let parser = MarkdownParser::new(&arena);
    if cli.acknowledgements {
        display_acknowledgements();
        return Ok(());
    }
    let path = cli.path.expect("no path");
    let resources_path = path.parent().unwrap_or(Path::new("/"));
    let resources = Resources::new(resources_path);
    let typst = TypstRender::new(config.typst.ppi);
    if cli.export_pdf || cli.generate_pdf_metadata {
        let mut exporter = Exporter::new(parser, &default_theme, resources, typst, themes);
        if cli.export_pdf {
            exporter.export_pdf(&path)?;
        } else {
            let meta = exporter.generate_metadata(&path)?;
            println!("{}", serde_json::to_string_pretty(&meta)?);
        }
    } else {
        let commands = CommandSource::new(&path);
        let presenter = Presenter::new(&default_theme, commands, parser, resources, typst, themes, mode);
        presenter.present(&path)?;
    }
    Ok(())
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
