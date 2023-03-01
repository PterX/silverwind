use diesel::r2d2::ConnectionManager;
use diesel::r2d2::{self, ManageConnection};
use diesel::MysqlConnection;
use dotenvy::dotenv;
use lazy_static::lazy_static;
use std::env;
pub type DbConnection = r2d2::PooledConnection<ConnectionManager<MysqlConnection>>;
pub type Pool = r2d2::Pool<ConnectionManager<MysqlConnection>>;

use std::panic;
use std::sync::Mutex;

use tokio::time;

#[derive(Debug, Clone)]
pub struct ConnectionPool {
    pub pool: Option<Pool>,
}
lazy_static! {
    pub static ref CONNECTION_POOL: Mutex<ConnectionPool> =
        Mutex::new(ConnectionPool { pool: None });
}
impl ConnectionPool {
    fn get(&mut self) -> Result<DbConnection, r2d2::PoolError> {
        self.pool.clone().unwrap().get()
    }
}

pub async fn schedule_task_connection_pool() {
    let mut interval = time::interval(time::Duration::from_secs(5));
    loop {
        match connect_with_database() {
            Ok(()) => debug!("check database status is ok"),
            Err(err) => error!("connect_with_database is error,the error is :{}", err),
        }
        interval.tick().await;
    }
}
fn connect_with_database() -> Result<(), anyhow::Error> {
    let connection_pool = match CONNECTION_POOL.lock() {
        Ok(pool) => pool.to_owned().clone(),
        Err(err) => {
            error!("error is {}", err);
            return Err(anyhow!(err.to_string()));
        }
    };
    let option_connection_pool = connection_pool.pool;
    if option_connection_pool.is_none() {
        return create_connection();
    }
    let pool = option_connection_pool.unwrap();
    let state = pool.clone().state();
    if state.connections == 0 {
        return create_connection();
    }

    Ok(())
}
fn create_connection() -> Result<(), anyhow::Error> {
    let new_connection_pool = match create_connection_pool() {
        Err(err) => return Err(anyhow!(err.to_string())),
        Ok(rw_connect_pool) => rw_connect_pool,
    };
    let new_connection_pool = new_connection_pool.lock().unwrap().to_owned().clone();
    let new_pool = new_connection_pool.pool.clone();
    if new_pool.is_none() {
        return Err(anyhow!("new pool is empty!"));
    }
    let state = new_pool.unwrap().state();
    if state.connections == 0 {
        return Err(anyhow!("There are no connections in the pool!"));
    }
    let mut old_lock = match CONNECTION_POOL.lock() {
        Ok(lock) => lock,
        Err(err) => return Err(anyhow!(err.to_string())),
    };
    *old_lock = ConnectionPool {
        pool: new_connection_pool.pool.clone(),
    };

    Ok(())
}
/**
 *The Pool::builder() will take a lot of the time.So I check the connection first
 */
fn create_connection_pool() -> Result<Mutex<ConnectionPool>, anyhow::Error> {
    dotenv().ok();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    info!("Database URL: {}", database_url);
    let manager = ConnectionManager::<MysqlConnection>::new(database_url);
    let result_test_connection = manager.connect();
    if let Err(e) = result_test_connection {
        return Err(anyhow!(e.to_string()));
    } else {
        let mut pool = panic::catch_unwind(|| {
            return Pool::builder()
                .min_idle(Some(5))
                .max_size(10)
                .build(manager);
        });
        if pool.is_err() || pool.as_mut().unwrap().is_err() {
            if pool.is_err() {
                error!("panic when creating the pool")
            } else {
                error!("error is {}", pool.unwrap().unwrap_err())
            }
            return Ok(Mutex::new(ConnectionPool { pool: None }));
        } else {
            return Ok(Mutex::new(ConnectionPool {
                pool: Some(pool.unwrap().unwrap()),
            }));
        }
    }
}
pub fn get_connection() -> Result<DbConnection, anyhow::Error> {
    let connection_pool = match CONNECTION_POOL.lock() {
        Ok(pool) => pool.to_owned(),
        Err(e) => return Err(anyhow!(e.to_string())),
    };
    if connection_pool.pool.is_none() {
        return Err(anyhow!("the connection pool is not ready"));
    }

    let pool = connection_pool.pool.unwrap();
    let state = pool.clone().state();
    if state.connections == 0 {
        return Err(anyhow!("There are no connections in the pool."));
    }
    let result = pool.clone().get();
    match result {
        Ok(conn) => Ok(conn),
        Err(err) => return Err(anyhow!(err.to_string())),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    // fn run_test<T>(test: T) -> ()
    // where
    //     T: FnOnce() -> () + panic::UnwindSafe,
    // {
    //     setup();
    //     let result = panic::catch_unwind(|| test());
    //     teardown();
    //     assert!(result.is_ok())
    // }
    #[test]
    fn test_get_connection_fail() {
        let result_connection = get_connection();
        assert_eq!(result_connection.is_err(), true);
    }
}