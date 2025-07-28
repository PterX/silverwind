use clap::Parser;

use super::app_config::AppConfig;
use clap::Args;
use clap::Subcommand;
use clap::ValueEnum;
use std::path::PathBuf;
use std::sync::Arc;

use std::sync::Mutex;
#[derive(ValueEnum, Debug, Clone)]
pub enum InputFormat {
    Openapi,
    Swagger,
}
#[derive(Parser, Debug, Clone)]
#[command(name = "Spire", version = crate_version!(), about = concat!("The Spire API Gateway v", crate_version!()), long_about = None) ]
pub struct Cli {
    #[arg(short = 'f', long, default_value = "config.yaml")]
    pub config_path: String,
    #[command(subcommand)]
    pub command: Option<Commands>,
}
#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    #[command(
        visible_alias = "conv",
        about = "Converts an OpenAPI/Swagger file into a gateway configuration"
    )]
    Convert(ConvertArgs),
}

#[derive(Args, Debug, Clone)]
pub struct ConvertArgs {
    #[arg(required = true, value_name = "INPUT_FILE")]
    pub input_file: PathBuf,

    #[arg(short = 'o', long, value_name = "OUTPUT_FILE")]
    pub output_file: Option<PathBuf>,
    #[arg(long, value_enum, default_value_t = InputFormat::Openapi)]
    pub format: InputFormat,
}
#[derive(Clone)]
pub struct SharedConfig {
    pub shared_data: Arc<Mutex<AppConfig>>,
}
impl SharedConfig {
    pub fn from_app_config(app_config: AppConfig) -> Self {
        Self {
            shared_data: Arc::new(Mutex::new(app_config)),
        }
    }
}
