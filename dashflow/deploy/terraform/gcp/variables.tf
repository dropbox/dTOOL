# DashFlow GCP Terraform Variables

# General
variable "project_id" {
  description = "GCP project ID"
  type        = string
}

variable "project_name" {
  description = "Project name used for resource naming"
  type        = string
  default     = "dashflow"
}

variable "environment" {
  description = "Environment (dev, staging, production)"
  type        = string
  validation {
    condition     = contains(["dev", "staging", "production"], var.environment)
    error_message = "Environment must be dev, staging, or production."
  }
}

variable "region" {
  description = "GCP region"
  type        = string
  default     = "us-west1"
}

# Network Configuration
variable "nodes_subnet_cidr" {
  description = "CIDR for nodes subnet"
  type        = string
  default     = "10.0.0.0/20"
}

variable "pods_subnet_cidr" {
  description = "CIDR for pods secondary range"
  type        = string
  default     = "10.4.0.0/14"
}

variable "services_subnet_cidr" {
  description = "CIDR for services secondary range"
  type        = string
  default     = "10.8.0.0/20"
}

variable "master_cidr" {
  description = "CIDR for GKE master"
  type        = string
  default     = "172.16.0.0/28"
}

# GKE Configuration - General Nodes
variable "general_machine_type" {
  description = "Machine type for general node pool"
  type        = string
  default     = "e2-standard-4"
}

variable "general_node_count" {
  description = "Initial node count for general pool"
  type        = number
  default     = 3
}

variable "general_min_nodes" {
  description = "Minimum nodes for general pool"
  type        = number
  default     = 2
}

variable "general_max_nodes" {
  description = "Maximum nodes for general pool"
  type        = number
  default     = 10
}

# GKE Configuration - Streaming Nodes
variable "streaming_machine_type" {
  description = "Machine type for streaming node pool"
  type        = string
  default     = "e2-standard-8"
}

variable "streaming_node_count" {
  description = "Initial node count for streaming pool"
  type        = number
  default     = 2
}

variable "streaming_min_nodes" {
  description = "Minimum nodes for streaming pool"
  type        = number
  default     = 1
}

variable "streaming_max_nodes" {
  description = "Maximum nodes for streaming pool"
  type        = number
  default     = 5
}

# Redis (Memorystore) Configuration
variable "enable_redis" {
  description = "Enable Memorystore Redis"
  type        = bool
  default     = true
}

variable "redis_memory_size_gb" {
  description = "Redis memory size in GB"
  type        = number
  default     = 4
}

# PostgreSQL (Cloud SQL) Configuration
variable "enable_postgres" {
  description = "Enable Cloud SQL PostgreSQL"
  type        = bool
  default     = true
}

variable "postgres_tier" {
  description = "Cloud SQL instance tier"
  type        = string
  default     = "db-custom-2-4096"
}

variable "postgres_disk_size" {
  description = "Cloud SQL disk size in GB"
  type        = number
  default     = 20
}

variable "postgres_username" {
  description = "PostgreSQL username"
  type        = string
  default     = "dashflow_admin"
  sensitive   = true
}

variable "postgres_password" {
  description = "PostgreSQL password"
  type        = string
  sensitive   = true
}

# DashFlow Application
variable "dashflow_namespace" {
  description = "Kubernetes namespace for DashFlow"
  type        = string
  default     = "dashflow"
}

variable "dashflow_version" {
  description = "DashFlow version/image tag to deploy"
  type        = string
  default     = "1.11.1"
}
