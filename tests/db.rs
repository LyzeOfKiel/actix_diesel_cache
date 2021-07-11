use std::process::Command;

#[derive(Debug)]
pub struct PgDb {
    pub url: String,
    pub container_name: String,
}

impl PgDb {
    pub fn new() -> Self {
        let id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
            % 10000;

        let user = "postgres";
        let password = "smartscales";
        let url = "127.0.0.1";
        let port = id + 1000;
        let db_name = "smartscales";

        let container_name = format!("smartscales-postgres-test-{}", id);
        let url = format!(
            "postgres://{}:{}@{}:{}/{}",
            user, password, url, port, db_name
        );

        #[rustfmt::skip]
        let output = Command::new("docker")
            .args(&[
                "run",
                "-d",
                "--rm",
                "-p", &format!("127.0.0.1:{}:5432", port),
                "-e", &format!("{}={}", "POSTGRES_USER",     user),
                "-e", &format!("{}={}", "POSTGRES_PASSWORD", password),
                "-e", &format!("{}={}", "POSTGRES_DB",       db_name),
                "--name", &container_name,
                "postgres:13-alpine",
            ])
            .output()
            .expect("failed to start postgres");
        assert!(output.status.success());

        Self {
            url,
            container_name,
        }
    }

    pub fn kill(&self) {
        let output = Command::new("docker")
            .args(&["kill", self.container_name.as_str()])
            .output()
            .expect("failed to kill postgres");
        println!("{:?}", output);
        assert!(output.status.success());
    }
}

impl Drop for PgDb {
    fn drop(&mut self) {
        Command::new("docker")
            .args(&["stop", &self.container_name])
            .output()
            .unwrap();
        Command::new("docker")
            .args(&["rm", &self.container_name])
            .output()
            .unwrap();
    }
}
