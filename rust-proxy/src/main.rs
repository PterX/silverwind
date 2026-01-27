mod app;
mod command;
mod configuration_service;
mod constants;
mod control_plane;
mod health_check;
mod middleware;
mod monitor;
mod proxy;
mod utils;
mod vojo;
use crate::app::run::main_with_error;
#[macro_use]
extern crate clap;
#[macro_use]
extern crate tracing;
#[tokio::main]
async fn main() {
    if let Err(e) = main_with_error().await {
        eprint!("{}", e);
    }
}
