# DashFlow GCP Terraform Outputs

# Network Outputs
output "vpc_name" {
  description = "VPC network name"
  value       = google_compute_network.this.name
}

output "subnet_name" {
  description = "Subnet name"
  value       = google_compute_subnetwork.nodes.name
}

# GKE Outputs
output "gke_cluster_name" {
  description = "GKE cluster name"
  value       = google_container_cluster.this.name
}

output "gke_cluster_endpoint" {
  description = "GKE cluster endpoint"
  value       = google_container_cluster.this.endpoint
}

output "gke_kubeconfig_command" {
  description = "Command to configure kubectl"
  value       = "gcloud container clusters get-credentials ${google_container_cluster.this.name} --region ${var.region} --project ${var.project_id}"
}

# Redis Outputs
output "redis_host" {
  description = "Memorystore Redis host"
  value       = var.enable_redis ? google_redis_instance.this[0].host : null
}

output "redis_port" {
  description = "Memorystore Redis port"
  value       = var.enable_redis ? google_redis_instance.this[0].port : null
}

# PostgreSQL Outputs
output "postgres_connection_name" {
  description = "Cloud SQL connection name"
  value       = var.enable_postgres ? google_sql_database_instance.this[0].connection_name : null
}

output "postgres_private_ip" {
  description = "Cloud SQL private IP"
  value       = var.enable_postgres ? google_sql_database_instance.this[0].private_ip_address : null
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
    gke_kubeconfig = "gcloud container clusters get-credentials ${google_container_cluster.this.name} --region ${var.region} --project ${var.project_id}"
    redis_url      = var.enable_redis ? "redis://${google_redis_instance.this[0].host}:${google_redis_instance.this[0].port}" : "N/A"
    postgres_url   = var.enable_postgres ? "postgresql://${var.postgres_username}@${google_sql_database_instance.this[0].private_ip_address}:5432/dashflow" : "N/A"
  }
  sensitive = true
}
