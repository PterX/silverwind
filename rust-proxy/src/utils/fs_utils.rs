use crate::app_error;
use crate::AppError;
use home::home_dir;
use std::path::PathBuf;
pub fn get_domain_path(domain_name: &str) -> Result<PathBuf, AppError> {
    let path = home_dir()
        .ok_or_else(|| app_error!("Failed to get user home directory"))?
        .join(".spire")
        .join("domains")
        .join(domain_name);

    Ok(path)
}
