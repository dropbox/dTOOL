# DashFlow AWS Terraform Outputs

# VPC Outputs
output "vpc_id" {
  description = "VPC ID"
  value       = module.vpc.vpc_id
}

output "private_subnet_ids" {
  description = "Private subnet IDs"
  value       = module.vpc.private_subnet_ids
}

output "public_subnet_ids" {
  description = "Public subnet IDs"
  value       = module.vpc.public_subnet_ids
}

# EKS Outputs
output "eks_cluster_name" {
  description = "EKS cluster name"
  value       = module.eks.cluster_name
}

output "eks_cluster_endpoint" {
  description = "EKS cluster API endpoint"
  value       = module.eks.cluster_endpoint
}

output "eks_cluster_arn" {
  description = "EKS cluster ARN"
  value       = module.eks.cluster_arn
}

output "eks_kubeconfig_command" {
  description = "Command to update kubeconfig"
  value       = "aws eks update-kubeconfig --name ${module.eks.cluster_name} --region ${var.aws_region}"
}

output "eks_oidc_provider_arn" {
  description = "EKS OIDC provider ARN for IRSA"
  value       = module.eks.oidc_provider_arn
}

# Redis Outputs
output "redis_endpoint" {
  description = "ElastiCache Redis endpoint"
  value       = var.enable_redis ? module.elasticache[0].endpoint : null
}

output "redis_port" {
  description = "ElastiCache Redis port"
  value       = var.enable_redis ? 6379 : null
}

# Kafka Outputs
output "kafka_bootstrap_brokers" {
  description = "MSK bootstrap broker endpoints (TLS)"
  value       = var.enable_kafka ? module.msk[0].bootstrap_brokers_tls : null
}

output "kafka_zookeeper_connect" {
  description = "MSK Zookeeper connection string"
  value       = var.enable_kafka ? module.msk[0].zookeeper_connect_string : null
}

# PostgreSQL Outputs
output "postgres_endpoint" {
  description = "RDS PostgreSQL endpoint"
  value       = var.enable_postgres ? module.rds[0].endpoint : null
}

output "postgres_port" {
  description = "RDS PostgreSQL port"
  value       = var.enable_postgres ? 5432 : null
}

# DashFlow Outputs
output "dashflow_namespace" {
  description = "Kubernetes namespace where DashFlow is deployed"
  value       = var.dashflow_namespace
}

output "dashflow_version" {
  description = "Deployed DashFlow version"
  value       = var.dashflow_version
}

# Connection Information
output "connection_info" {
  description = "Connection information for DashFlow components"
  value = {
    eks_kubeconfig = "aws eks update-kubeconfig --name ${module.eks.cluster_name} --region ${var.aws_region}"
    redis_url      = var.enable_redis ? "redis://${module.elasticache[0].endpoint}:6379" : "N/A"
    kafka_brokers  = var.enable_kafka ? module.msk[0].bootstrap_brokers_tls : "N/A"
    postgres_url   = var.enable_postgres ? "postgresql://${var.rds_username}@${module.rds[0].endpoint}:5432/dashflow" : "N/A"
  }
  sensitive = true
}
