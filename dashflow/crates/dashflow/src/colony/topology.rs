// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! System topology and architecture information.
//!
//! Provides information about the system's hardware architecture, deployment options,
//! and network configuration.

use serde::{Deserialize, Serialize};

/// System topology information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemTopology {
    /// Hostname of the machine
    pub hostname: String,

    /// Operating system name
    pub os: String,

    /// OS version
    pub os_version: String,

    /// CPU architecture (x86_64, aarch64, etc.)
    pub architecture: String,

    /// Number of NUMA nodes (for multi-socket systems)
    pub numa_nodes: Vec<NumaNode>,

    /// Network interfaces
    pub network_interfaces: Vec<NetworkInterface>,

    /// Available container runtime
    pub container_runtime: Option<ContainerRuntime>,

    /// Kubernetes available
    pub kubernetes_available: bool,

    /// Current process ID
    pub pid: u32,

    /// Current working directory
    pub cwd: String,
}

impl SystemTopology {
    /// Get available spawn/deployment options based on topology.
    pub fn spawn_options(&self) -> Vec<SpawnOption> {
        let mut options = vec![SpawnOption::Process]; // Always available

        if self.container_runtime.is_some() {
            options.push(SpawnOption::Docker);
        }

        if self.kubernetes_available {
            options.push(SpawnOption::Kubernetes);
        }

        options
    }

    /// Get the best deployment option for a given requirement.
    pub fn best_deployment_option(&self, requirements: &DeploymentOption) -> Option<SpawnOption> {
        match requirements {
            DeploymentOption::Any => Some(SpawnOption::Process),
            DeploymentOption::Isolated => {
                if self.container_runtime.is_some() {
                    Some(SpawnOption::Docker)
                } else {
                    Some(SpawnOption::Process)
                }
            }
            DeploymentOption::Distributed => {
                if self.kubernetes_available {
                    Some(SpawnOption::Kubernetes)
                } else if self.container_runtime.is_some() {
                    Some(SpawnOption::Docker)
                } else {
                    Some(SpawnOption::Process)
                }
            }
            DeploymentOption::Local => Some(SpawnOption::Process),
        }
    }

    /// Check if the system has multiple NUMA nodes.
    pub fn is_numa(&self) -> bool {
        self.numa_nodes.len() > 1
    }

    /// Get total CPU cores across all NUMA nodes.
    pub fn total_cpu_cores(&self) -> u32 {
        self.numa_nodes.iter().map(|n| n.cpu_cores).sum()
    }

    /// Get total memory across all NUMA nodes (MB).
    pub fn total_memory_mb(&self) -> u64 {
        self.numa_nodes.iter().map(|n| n.memory_mb).sum()
    }
}

/// NUMA node information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NumaNode {
    /// Node ID
    pub id: u32,

    /// CPU cores in this node
    pub cpu_cores: u32,

    /// CPU IDs assigned to this node
    pub cpu_ids: Vec<u32>,

    /// Memory in this node (MB)
    pub memory_mb: u64,
}

/// Network interface information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterface {
    /// Interface name (eth0, en0, etc.)
    pub name: String,

    /// MAC address
    pub mac_address: Option<String>,

    /// IPv4 addresses
    pub ipv4_addresses: Vec<String>,

    /// IPv6 addresses
    pub ipv6_addresses: Vec<String>,

    /// Is this the primary interface?
    pub is_primary: bool,

    /// Link speed in Mbps
    pub link_speed_mbps: Option<u32>,

    /// Is the interface up?
    pub is_up: bool,
}

/// Container runtime information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerRuntime {
    /// Runtime type (docker, podman, containerd)
    pub runtime_type: String,

    /// Runtime version
    pub version: String,

    /// Is the runtime available and running?
    pub available: bool,

    /// Socket path
    pub socket_path: Option<String>,
}

/// Spawn/deployment options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpawnOption {
    /// Spawn as child process (always available)
    Process,

    /// Spawn in Docker container
    Docker,

    /// Spawn as Kubernetes job/pod
    Kubernetes,
}

impl SpawnOption {
    /// Get the isolation level provided by this option.
    pub fn isolation_level(&self) -> u8 {
        match self {
            Self::Process => 1,
            Self::Docker => 2,
            Self::Kubernetes => 3,
        }
    }

    /// Get human-readable name.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Process => "Process",
            Self::Docker => "Docker Container",
            Self::Kubernetes => "Kubernetes Pod",
        }
    }
}

/// Deployment preference/requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DeploymentOption {
    /// Any deployment option is acceptable
    #[default]
    Any,

    /// Prefer isolated deployment (container)
    Isolated,

    /// Prefer distributed deployment (kubernetes)
    Distributed,

    /// Must be local process
    Local,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_topology(with_docker: bool, with_kubernetes: bool) -> SystemTopology {
        SystemTopology {
            hostname: "test-host".to_string(),
            os: "linux".to_string(),
            os_version: "6.1.0".to_string(),
            architecture: "x86_64".to_string(),
            numa_nodes: vec![NumaNode {
                id: 0,
                cpu_cores: 8,
                cpu_ids: (0..8).collect(),
                memory_mb: 16384,
            }],
            network_interfaces: vec![NetworkInterface {
                name: "eth0".to_string(),
                mac_address: Some("00:11:22:33:44:55".to_string()),
                ipv4_addresses: vec!["192.168.1.100".to_string()],
                ipv6_addresses: vec![],
                is_primary: true,
                link_speed_mbps: Some(1000),
                is_up: true,
            }],
            container_runtime: if with_docker {
                Some(ContainerRuntime {
                    runtime_type: "docker".to_string(),
                    version: "24.0.0".to_string(),
                    available: true,
                    socket_path: Some("/var/run/docker.sock".to_string()),
                })
            } else {
                None
            },
            kubernetes_available: with_kubernetes,
            pid: 1234,
            cwd: "/home/user".to_string(),
        }
    }

    #[test]
    fn test_spawn_options_process_only() {
        let topology = create_test_topology(false, false);
        let options = topology.spawn_options();
        assert_eq!(options, vec![SpawnOption::Process]);
    }

    #[test]
    fn test_spawn_options_with_docker() {
        let topology = create_test_topology(true, false);
        let options = topology.spawn_options();
        assert_eq!(options, vec![SpawnOption::Process, SpawnOption::Docker]);
    }

    #[test]
    fn test_spawn_options_with_all() {
        let topology = create_test_topology(true, true);
        let options = topology.spawn_options();
        assert_eq!(
            options,
            vec![
                SpawnOption::Process,
                SpawnOption::Docker,
                SpawnOption::Kubernetes
            ]
        );
    }

    #[test]
    fn test_best_deployment_option() {
        let topology = create_test_topology(true, false);

        assert_eq!(
            topology.best_deployment_option(&DeploymentOption::Any),
            Some(SpawnOption::Process)
        );

        assert_eq!(
            topology.best_deployment_option(&DeploymentOption::Isolated),
            Some(SpawnOption::Docker)
        );

        assert_eq!(
            topology.best_deployment_option(&DeploymentOption::Local),
            Some(SpawnOption::Process)
        );

        // Without kubernetes, distributed falls back to docker
        assert_eq!(
            topology.best_deployment_option(&DeploymentOption::Distributed),
            Some(SpawnOption::Docker)
        );

        // With kubernetes
        let k8s_topology = create_test_topology(true, true);
        assert_eq!(
            k8s_topology.best_deployment_option(&DeploymentOption::Distributed),
            Some(SpawnOption::Kubernetes)
        );
    }

    #[test]
    fn test_numa_detection() {
        let single_numa = create_test_topology(false, false);
        assert!(!single_numa.is_numa());

        let mut multi_numa = create_test_topology(false, false);
        multi_numa.numa_nodes.push(NumaNode {
            id: 1,
            cpu_cores: 8,
            cpu_ids: (8..16).collect(),
            memory_mb: 16384,
        });
        assert!(multi_numa.is_numa());
    }

    #[test]
    fn test_total_resources() {
        let mut topology = create_test_topology(false, false);
        topology.numa_nodes.push(NumaNode {
            id: 1,
            cpu_cores: 8,
            cpu_ids: (8..16).collect(),
            memory_mb: 16384,
        });

        assert_eq!(topology.total_cpu_cores(), 16);
        assert_eq!(topology.total_memory_mb(), 32768);
    }

    #[test]
    fn test_spawn_option_properties() {
        assert_eq!(SpawnOption::Process.isolation_level(), 1);
        assert_eq!(SpawnOption::Docker.isolation_level(), 2);
        assert_eq!(SpawnOption::Kubernetes.isolation_level(), 3);

        assert_eq!(SpawnOption::Process.name(), "Process");
        assert_eq!(SpawnOption::Docker.name(), "Docker Container");
        assert_eq!(SpawnOption::Kubernetes.name(), "Kubernetes Pod");
    }
}
