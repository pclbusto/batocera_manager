use crate::net::{self, SshConfig};

pub fn test_connection(config: &SshConfig) -> Result<String, String> {
    net::test_connection(config)
}
