use git_prole::App;
use git_prole::Config;

fn main() -> miette::Result<()> {
    let config = Config::new()?;
    App::new(config).run()
}
