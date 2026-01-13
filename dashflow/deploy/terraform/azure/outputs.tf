# DashFlow Azure Terraform Outputs

# Resource Group Outputs
output "resource_group_name" {
  description = "Resource group name"
  value       = azurerm_resource_group.this.name
}

# Network Outputs
output "vnet_name" {
  description = "VNet name"
  value       = azurerm_virtual_network.this.name
}

output "aks_subnet_id" {
  description = "AKS subnet ID"
  value       = azurerm_subnet.aks.id
}

# AKS Outputs
output "aks_cluster_name" {
  description = "AKS cluster name"
  value       = azurerm_kubernetes_cluster.this.name
}

output "aks_cluster_fqdn" {
  description = "AKS cluster FQDN"
  value       = azurerm_kubernetes_cluster.this.fqdn
}

output "aks_kubeconfig_command" {
  description = "Command to configure kubectl"
  value       = "az aks get-credentials --resource-group ${azurerm_resource_group.this.name} --name ${azurerm_kubernetes_cluster.this.name}"
}

output "aks_identity_principal_id" {
  description = "AKS managed identity principal ID"
  value       = azurerm_kubernetes_cluster.this.identity[0].principal_id
}

# Redis Outputs
output "redis_hostname" {
  description = "Redis hostname"
  value       = var.enable_redis ? azurerm_redis_cache.this[0].hostname : null
}

output "redis_port" {
  description = "Redis SSL port"
  value       = var.enable_redis ? azurerm_redis_cache.this[0].ssl_port : null
}

output "redis_primary_key" {
  description = "Redis primary access key"
  value       = var.enable_redis ? azurerm_redis_cache.this[0].primary_access_key : null
  sensitive   = true
}

# PostgreSQL Outputs
output "postgres_fqdn" {
  description = "PostgreSQL FQDN"
  value       = var.enable_postgres ? azurerm_postgresql_flexible_server.this[0].fqdn : null
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
    aks_kubeconfig = "az aks get-credentials --resource-group ${azurerm_resource_group.this.name} --name ${azurerm_kubernetes_cluster.this.name}"
    redis_url      = var.enable_redis ? "rediss://:${azurerm_redis_cache.this[0].primary_access_key}@${azurerm_redis_cache.this[0].hostname}:${azurerm_redis_cache.this[0].ssl_port}" : "N/A"
    postgres_url   = var.enable_postgres ? "postgresql://${var.postgres_username}@${azurerm_postgresql_flexible_server.this[0].fqdn}:5432/dashflow?sslmode=require" : "N/A"
  }
  sensitive = true
}
