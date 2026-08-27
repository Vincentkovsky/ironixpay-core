use ironix_pay::migration::Migrator;
use sea_orm::{Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;
use testcontainers::runners::AsyncRunner;
use testcontainers::ContainerAsync;
use testcontainers_modules::postgres::Postgres;

/// Helper struct to hold both the DB connection and the container
/// The container is dropped when this struct is dropped, cleaning up resources.
pub struct TestDb {
    pub conn: DatabaseConnection,
    pub _container: ContainerAsync<Postgres>,
}

pub async fn setup_test_db() -> TestDb {
    // Start Postgres container
    let container = Postgres::default().start().await.unwrap();

    // Get connection string
    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let db_url = format!("postgres://postgres:postgres@{}:{}/postgres", host, port);

    // Connect to DB
    let db = Database::connect(&db_url).await.unwrap();

    // Run migrations
    Migrator::up(&db, None).await.unwrap();

    TestDb {
        conn: db,
        _container: container,
    }
}

pub const TEST_ENCRYPTION_KEY: [u8; 32] = [1u8; 32];
// 32 bytes of 0x01 = 64 characters of "01"
pub const TEST_ENCRYPTION_KEY_HEX: &str =
    "0101010101010101010101010101010101010101010101010101010101010101";

pub fn init_logger() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("debug")
        .with_test_writer()
        .try_init();
}
