pub const DOCKER_IMAGE: &str = "dawdle-home";
pub const DOCKER_TAG: &str = "latest";
pub const DOCKER_CONTAINER_PREFIX: &str = "dawdle-home-";

#[cfg(target_os = "darwin")]
pub const DOCKER_SOCKET_MACOS: &str = "unix:///Users/henry/.colima/default/docker.sock";
