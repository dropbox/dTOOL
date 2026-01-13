# DashFlow AWS Terraform Variables

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

variable "aws_region" {
  description = "AWS region for resources"
  type        = string
  default     = "us-west-2"
}

# VPC Configuration
variable "vpc_cidr" {
  description = "CIDR block for VPC"
  type        = string
  default     = "10.0.0.0/16"
}

variable "enable_nat_gateway" {
  description = "Enable NAT Gateway for private subnets"
  type        = bool
  default     = true
}

# EKS Configuration
variable "eks_cluster_version" {
  description = "Kubernetes version for EKS cluster"
  type        = string
  default     = "1.28"
}

variable "eks_node_groups" {
  description = "EKS node group configurations"
  type = map(object({
    instance_types = list(string)
    min_size       = number
    max_size       = number
    desired_size   = number
    disk_size      = number
    labels         = map(string)
    taints = list(object({
      key    = string
      value  = string
      effect = string
    }))
  }))
  default = {
    general = {
      instance_types = ["m5.large"]
      min_size       = 2
      max_size       = 10
      desired_size   = 3
      disk_size      = 50
      labels         = { "workload" = "general" }
      taints         = []
    }
    streaming = {
      instance_types = ["m5.xlarge"]
      min_size       = 1
      max_size       = 5
      desired_size   = 2
      disk_size      = 100
      labels         = { "workload" = "streaming" }
      taints = [{
        key    = "workload"
        value  = "streaming"
        effect = "NO_SCHEDULE"
      }]
    }
  }
}

# Redis (ElastiCache) Configuration
variable "enable_redis" {
  description = "Enable ElastiCache Redis cluster"
  type        = bool
  default     = true
}

variable "redis_node_type" {
  description = "ElastiCache node type"
  type        = string
  default     = "cache.t3.medium"
}

# Kafka (MSK) Configuration
variable "enable_kafka" {
  description = "Enable MSK Kafka cluster"
  type        = bool
  default     = true
}

variable "kafka_version" {
  description = "Kafka version for MSK"
  type        = string
  default     = "3.5.1"
}

variable "kafka_broker_instance_type" {
  description = "MSK broker instance type"
  type        = string
  default     = "kafka.m5.large"
}

# PostgreSQL (RDS) Configuration
variable "enable_postgres" {
  description = "Enable RDS PostgreSQL instance"
  type        = bool
  default     = true
}

variable "rds_instance_class" {
  description = "RDS instance class"
  type        = string
  default     = "db.t3.medium"
}

variable "rds_allocated_storage" {
  description = "RDS allocated storage in GB"
  type        = number
  default     = 20
}

variable "rds_username" {
  description = "RDS master username"
  type        = string
  default     = "dashflow_admin"
  sensitive   = true
}

variable "rds_password" {
  description = "RDS master password"
  type        = string
  sensitive   = true
}

# Observability Configuration
variable "enable_xray" {
  description = "Enable AWS X-Ray tracing"
  type        = bool
  default     = true
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
