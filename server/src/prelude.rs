use dotenv::var;
use log::LevelFilter;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::LazyLock;

pub static DATA_DIR_PATH: LazyLock<PathBuf> =
    LazyLock::new(|| dirs::data_dir().unwrap().join("manage_rs").join("server"));

pub type Res = eyre::Result<()>;

pub async fn init_logger() -> Res {
    fern::Dispatch::new()
        .format(|out, message, record| {
            let datetime: chrono::DateTime<chrono::Utc> = std::time::SystemTime::now().into();
            let formatted_time = datetime
                .format_with_items(chrono::format::StrftimeItems::new("%H:%M:%S"))
                .to_string();
            out.finish(format_args!(
                "[{} - {} ({})] {}",
                formatted_time,
                record.level(),
                record.target(),
                message
            ))
        })
        .level(LevelFilter::from_str(&var("LOG").unwrap_or("info".into()))?)
        .chain(std::io::stdout())
        .apply()?;

    Ok(())
}
