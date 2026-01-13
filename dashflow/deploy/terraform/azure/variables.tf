# DashFlow Azure Terraform Variables

# General
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

variable "location" {
  description = "Azure region"
  type        = string
  default     = "westus2"
}

# Network Configuration
variable "vnet_cidr" {
  description = "CIDR for VNet"
  type        = string
  default     = "10.0.0.0/16"
}

variable "aks_subnet_cidr" {
  description = "CIDR for AKS subnet"
  type        = string
  default     = "10.0.0.0/20"
}

variable "services_subnet_cidr" {
  description = "CIDR for services subnet"
  type        = string
  default     = "10.0.16.0/24"
}

# AKS Configuration
variable "aks_version" {
  description = "Kubernetes version for AKS"
  type        = string
  default     = "1.28"
}

# General Node Pool
variable "general_vm_size" {
  description = "VM size for general node pool"
  type        = string
  default     = "Standard_D4s_v3"
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

# Streaming Node Pool
variable "streaming_vm_size" {
  description = "VM size for streaming node pool"
  type        = string
  default     = "Standard_D8s_v3"
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

# Redis Configuration
variable "enable_redis" {
  description = "Enable Azure Cache for Redis"
  type        = bool
  default     = true
}

variable "redis_capacity" {
  description = "Redis cache capacity"
  type        = number
  default     = 1
}

variable "redis_family" {
  description = "Redis cache family (C for Basic/Standard, P for Premium)"
  type        = string
  default     = "C"
}

variable "redis_sku" {
  description = "Redis cache SKU (Basic, Standard, Premium)"
  type        = string
  default     = "Standard"
}

# PostgreSQL Configuration
variable "enable_postgres" {
  description = "Enable Azure Database for PostgreSQL"
  type        = bool
  default     = true
}

variable "postgres_sku" {
  description = "PostgreSQL SKU name"
  type        = string
  default     = "GP_Standard_D2s_v3"
}

variable "postgres_storage_mb" {
  description = "PostgreSQL storage in MB"
  type        = number
  default     = 32768
}

variable "postgres_username" {
  description = "PostgreSQL admin username"
  type        = string
  default     = "dashflow_admin"
  sensitive   = true
}

variable "postgres_password" {
  description = "PostgreSQL admin password"
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
