use std::sync::Once;
use testcontainers::{clients::Cli, core::WaitFor, Container, Image};

static INIT: Once = Once::new();

#[derive(Debug, Default, Clone)]
pub struct HttpBin;

impl Image for HttpBin {
    type Args = ();

    fn name(&self) -> String {
        "kennethreitz/httpbin".to_owned()
    }

    fn tag(&self) -> String {
        "latest".to_owned()
    }

    fn ready_conditions(&self) -> Vec<WaitFor> {
        vec![WaitFor::message_on_stdout("Running on http://0.0.0.0:80/")]
    }
}

pub struct TestHttpBin {
    _container: Container<'static, HttpBin>,
    pub base_url: String,
}

impl TestHttpBin {
    pub fn new() -> Self {
        INIT.call_once(|| {
            env_logger::init();
        });

        let docker = Box::leak(Box::new(Cli::default()));
        let container = docker.run(HttpBin::default());
        let port = container.get_host_port_ipv4(80);
        
        let base_url = format!("http://127.0.0.1:{}", port);
        
        // Wait a bit for the container to be fully ready
        std::thread::sleep(std::time::Duration::from_millis(500));
        
        Self {
            _container: container,
            base_url,
        }
    }
}

// Global instance for reuse across tests
lazy_static::lazy_static! {
    pub static ref HTTPBIN: TestHttpBin = TestHttpBin::new();
}
