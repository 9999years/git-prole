use miette::IntoDiagnostic;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Layer;

pub fn install_tracing(filter_directives: &str) -> miette::Result<()> {
    let env_filter = tracing_subscriber::EnvFilter::try_new(filter_directives).into_diagnostic()?;

    let human_layer = tracing_human_layer::HumanLayer::new()
        .with_output_writer(std::io::stderr())
        .with_filter(env_filter);

    let registry = tracing_subscriber::registry();

    registry.with(human_layer).init();

    Ok(())
}
