use gufi::GufiOptions;

pub(crate) fn from_args() -> GufiOptions {
    let mut options = GufiOptions::from_args();
    options.runner.name.get_or_insert_with(|| env!("CARGO_PKG_NAME").into());
    options.runner.fleet_name.get_or_insert_with(|| "fastapi".into());
    options
}
