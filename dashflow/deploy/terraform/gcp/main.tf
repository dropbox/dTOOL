# DashFlow GCP Infrastructure
# Terraform configuration for deploying DashFlow on GKE

terraform {
  required_version = ">= 1.5.0"

  required_providers {
    google = {
      source  = "hashicorp/google"
      version = "~> 5.0"
    }
    google-beta = {
      source  = "hashicorp/google-beta"
      version = "~> 5.0"
    }
    kubernetes = {
      source  = "hashicorp/kubernetes"
      version = "~> 2.23"
    }
    helm = {
      source  = "hashicorp/helm"
      version = "~> 2.11"
    }
  }

  # Backend configuration - customize for your environment
  # backend "gcs" {
  #   bucket = "dashflow-terraform-state"
  #   prefix = "gcp/terraform.tfstate"
  # }
}

provider "google" {
  project = var.project_id
  region  = var.region
}

provider "google-beta" {
  project = var.project_id
  region  = var.region
}

# Enable required APIs
resource "google_project_service" "apis" {
  for_each = toset([
    "container.googleapis.com",
    "redis.googleapis.com",
    "sqladmin.googleapis.com",
    "servicenetworking.googleapis.com",
    "cloudresourcemanager.googleapis.com",
    "compute.googleapis.com",
  ])

  service            = each.key
  disable_on_destroy = false
}

# VPC Network
resource "google_compute_network" "this" {
  name                    = "${var.project_name}-${var.environment}-vpc"
  auto_create_subnetworks = false
  project                 = var.project_id

  depends_on = [google_project_service.apis]
}

# Subnets
resource "google_compute_subnetwork" "nodes" {
  name          = "${var.project_name}-${var.environment}-nodes"
  ip_cidr_range = var.nodes_subnet_cidr
  region        = var.region
  network       = google_compute_network.this.id
  project       = var.project_id

  secondary_ip_range {
    range_name    = "pods"
    ip_cidr_range = var.pods_subnet_cidr
  }

  secondary_ip_range {
    range_name    = "services"
    ip_cidr_range = var.services_subnet_cidr
  }

  private_ip_google_access = true
}

# Cloud Router for NAT
resource "google_compute_router" "this" {
  name    = "${var.project_name}-${var.environment}-router"
  region  = var.region
  network = google_compute_network.this.id
  project = var.project_id
}

# Cloud NAT
resource "google_compute_router_nat" "this" {
  name                               = "${var.project_name}-${var.environment}-nat"
  router                             = google_compute_router.this.name
  region                             = var.region
  nat_ip_allocate_option             = "AUTO_ONLY"
  source_subnetwork_ip_ranges_to_nat = "ALL_SUBNETWORKS_ALL_IP_RANGES"
  project                            = var.project_id

  log_config {
    enable = true
    filter = "ERRORS_ONLY"
  }
}

# GKE Cluster
resource "google_container_cluster" "this" {
  provider = google-beta

  name     = "${var.project_name}-${var.environment}"
  location = var.region
  project  = var.project_id

  # Remove default node pool
  remove_default_node_pool = true
  initial_node_count       = 1

  network    = google_compute_network.this.name
  subnetwork = google_compute_subnetwork.nodes.name

  ip_allocation_policy {
    cluster_secondary_range_name  = "pods"
    services_secondary_range_name = "services"
  }

  # Enable Workload Identity
  workload_identity_config {
    workload_pool = "${var.project_id}.svc.id.goog"
  }

  # Enable network policy
  network_policy {
    enabled  = true
    provider = "CALICO"
  }

  # Private cluster
  private_cluster_config {
    enable_private_nodes    = true
    enable_private_endpoint = false
    master_ipv4_cidr_block  = var.master_cidr
  }

  # Master authorized networks
  master_authorized_networks_config {
    cidr_blocks {
      cidr_block   = "0.0.0.0/0"
      display_name = "All"
    }
  }

  # Enable monitoring and logging
  logging_service    = "logging.googleapis.com/kubernetes"
  monitoring_service = "monitoring.googleapis.com/kubernetes"

  # Release channel
  release_channel {
    channel = var.environment == "production" ? "STABLE" : "REGULAR"
  }

  # Addons
  addons_config {
    http_load_balancing {
      disabled = false
    }
    horizontal_pod_autoscaling {
      disabled = false
    }
    network_policy_config {
      disabled = false
    }
  }

  # Maintenance window
  maintenance_policy {
    daily_maintenance_window {
      start_time = "03:00"
    }
  }

  depends_on = [google_project_service.apis]
}

# GKE Node Pool - General
resource "google_container_node_pool" "general" {
  name     = "general"
  location = var.region
  cluster  = google_container_cluster.this.name
  project  = var.project_id

  initial_node_count = var.general_node_count

  autoscaling {
    min_node_count = var.general_min_nodes
    max_node_count = var.general_max_nodes
  }

  node_config {
    machine_type = var.general_machine_type
    disk_size_gb = 50
    disk_type    = "pd-ssd"

    oauth_scopes = [
      "https://www.googleapis.com/auth/cloud-platform"
    ]

    labels = {
      workload = "general"
    }

    workload_metadata_config {
      mode = "GKE_METADATA"
    }

    shielded_instance_config {
      enable_secure_boot          = true
      enable_integrity_monitoring = true
    }
  }

  management {
    auto_repair  = true
    auto_upgrade = true
  }
}

# GKE Node Pool - Streaming
resource "google_container_node_pool" "streaming" {
  name     = "streaming"
  location = var.region
  cluster  = google_container_cluster.this.name
  project  = var.project_id

  initial_node_count = var.streaming_node_count

  autoscaling {
    min_node_count = var.streaming_min_nodes
    max_node_count = var.streaming_max_nodes
  }

  node_config {
    machine_type = var.streaming_machine_type
    disk_size_gb = 100
    disk_type    = "pd-ssd"

    oauth_scopes = [
      "https://www.googleapis.com/auth/cloud-platform"
    ]

    labels = {
      workload = "streaming"
    }

    taint {
      key    = "workload"
      value  = "streaming"
      effect = "NO_SCHEDULE"
    }

    workload_metadata_config {
      mode = "GKE_METADATA"
    }

    shielded_instance_config {
      enable_secure_boot          = true
      enable_integrity_monitoring = true
    }
  }

  management {
    auto_repair  = true
    auto_upgrade = true
  }
}

# Memorystore Redis
resource "google_redis_instance" "this" {
  count = var.enable_redis ? 1 : 0

  name           = "${var.project_name}-${var.environment}-redis"
  tier           = var.environment == "production" ? "STANDARD_HA" : "BASIC"
  memory_size_gb = var.redis_memory_size_gb
  region         = var.region
  project        = var.project_id

  authorized_network = google_compute_network.this.id
  connect_mode       = "PRIVATE_SERVICE_ACCESS"

  redis_version = "REDIS_7_0"

  transit_encryption_mode = "SERVER_AUTHENTICATION"

  labels = {
    project     = var.project_name
    environment = var.environment
  }

  depends_on = [google_project_service.apis]
}

# Cloud SQL PostgreSQL
resource "google_sql_database_instance" "this" {
  count = var.enable_postgres ? 1 : 0

  name             = "${var.project_name}-${var.environment}-postgres"
  database_version = "POSTGRES_15"
  region           = var.region
  project          = var.project_id

  settings {
    tier              = var.postgres_tier
    availability_type = var.environment == "production" ? "REGIONAL" : "ZONAL"
    disk_size         = var.postgres_disk_size
    disk_type         = "PD_SSD"

    ip_configuration {
      ipv4_enabled    = false
      private_network = google_compute_network.this.id
    }

    backup_configuration {
      enabled                        = true
      start_time                     = "03:00"
      point_in_time_recovery_enabled = var.environment == "production"
      backup_retention_settings {
        retained_backups = var.environment == "production" ? 30 : 7
      }
    }

    insights_config {
      query_insights_enabled  = var.environment == "production"
      record_application_tags = var.environment == "production"
      record_client_address   = false
    }
  }

  deletion_protection = var.environment == "production"

  depends_on = [google_project_service.apis]
}

resource "google_sql_database" "dashflow" {
  count = var.enable_postgres ? 1 : 0

  name     = "dashflow"
  instance = google_sql_database_instance.this[0].name
  project  = var.project_id
}

resource "google_sql_user" "dashflow" {
  count = var.enable_postgres ? 1 : 0

  name     = var.postgres_username
  instance = google_sql_database_instance.this[0].name
  password = var.postgres_password
  project  = var.project_id
}

# Configure Kubernetes provider with GKE credentials
data "google_client_config" "current" {}

provider "kubernetes" {
  host                   = "https://${google_container_cluster.this.endpoint}"
  token                  = data.google_client_config.current.access_token
  cluster_ca_certificate = base64decode(google_container_cluster.this.master_auth[0].cluster_ca_certificate)
}

# Configure Helm provider
provider "helm" {
  kubernetes {
    host                   = "https://${google_container_cluster.this.endpoint}"
    token                  = data.google_client_config.current.access_token
    cluster_ca_certificate = base64decode(google_container_cluster.this.master_auth[0].cluster_ca_certificate)
  }
}

# Deploy DashFlow using Helm
resource "helm_release" "dashflow" {
  name       = "dashflow"
  namespace  = var.dashflow_namespace
  chart      = "../../helm/dashflow"

  create_namespace = true
  wait             = true
  timeout          = 600

  values = [
    templatefile("${path.module}/values-${var.environment}.yaml", {
      redis_host        = var.enable_redis ? google_redis_instance.this[0].host : ""
      postgres_host     = var.enable_postgres ? google_sql_database_instance.this[0].private_ip_address : ""
      gcp_project       = var.project_id
      gcp_region        = var.region
    })
  ]

  set {
    name  = "image.tag"
    value = var.dashflow_version
  }

  set {
    name  = "global.environment"
    value = var.environment
  }

  depends_on = [
    google_container_node_pool.general,
    google_redis_instance.this,
    google_sql_database_instance.this
  ]
}
