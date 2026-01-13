# DashFlow Azure Infrastructure
# Terraform configuration for deploying DashFlow on AKS

terraform {
  required_version = ">= 1.5.0"

  required_providers {
    azurerm = {
      source  = "hashicorp/azurerm"
      version = "~> 3.80"
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
  # backend "azurerm" {
  #   resource_group_name  = "dashflow-terraform-state"
  #   storage_account_name = "dashflowtfstate"
  #   container_name       = "tfstate"
  #   key                  = "azure/terraform.tfstate"
  # }
}

provider "azurerm" {
  features {
    resource_group {
      prevent_deletion_if_contains_resources = false
    }
  }
}

# Resource Group
resource "azurerm_resource_group" "this" {
  name     = "${var.project_name}-${var.environment}-rg"
  location = var.location

  tags = local.common_tags
}

# Virtual Network
resource "azurerm_virtual_network" "this" {
  name                = "${var.project_name}-${var.environment}-vnet"
  location            = azurerm_resource_group.this.location
  resource_group_name = azurerm_resource_group.this.name
  address_space       = [var.vnet_cidr]

  tags = local.common_tags
}

# AKS Subnet
resource "azurerm_subnet" "aks" {
  name                 = "aks-subnet"
  resource_group_name  = azurerm_resource_group.this.name
  virtual_network_name = azurerm_virtual_network.this.name
  address_prefixes     = [var.aks_subnet_cidr]
}

# Services Subnet (Redis, PostgreSQL)
resource "azurerm_subnet" "services" {
  name                 = "services-subnet"
  resource_group_name  = azurerm_resource_group.this.name
  virtual_network_name = azurerm_virtual_network.this.name
  address_prefixes     = [var.services_subnet_cidr]

  delegation {
    name = "postgresql"
    service_delegation {
      name    = "Microsoft.DBforPostgreSQL/flexibleServers"
      actions = ["Microsoft.Network/virtualNetworks/subnets/join/action"]
    }
  }
}

# Log Analytics Workspace
resource "azurerm_log_analytics_workspace" "this" {
  name                = "${var.project_name}-${var.environment}-logs"
  location            = azurerm_resource_group.this.location
  resource_group_name = azurerm_resource_group.this.name
  sku                 = "PerGB2018"
  retention_in_days   = var.environment == "production" ? 90 : 30

  tags = local.common_tags
}

# AKS Cluster
resource "azurerm_kubernetes_cluster" "this" {
  name                = "${var.project_name}-${var.environment}"
  location            = azurerm_resource_group.this.location
  resource_group_name = azurerm_resource_group.this.name
  dns_prefix          = "${var.project_name}-${var.environment}"
  kubernetes_version  = var.aks_version

  default_node_pool {
    name                = "general"
    node_count          = var.general_node_count
    vm_size             = var.general_vm_size
    os_disk_size_gb     = 50
    vnet_subnet_id      = azurerm_subnet.aks.id
    enable_auto_scaling = true
    min_count           = var.general_min_nodes
    max_count           = var.general_max_nodes

    node_labels = {
      workload = "general"
    }
  }

  identity {
    type = "SystemAssigned"
  }

  network_profile {
    network_plugin    = "azure"
    network_policy    = "azure"
    load_balancer_sku = "standard"
  }

  oms_agent {
    log_analytics_workspace_id = azurerm_log_analytics_workspace.this.id
  }

  key_vault_secrets_provider {
    secret_rotation_enabled = true
  }

  tags = local.common_tags
}

# Streaming Node Pool
resource "azurerm_kubernetes_cluster_node_pool" "streaming" {
  name                  = "streaming"
  kubernetes_cluster_id = azurerm_kubernetes_cluster.this.id
  vm_size               = var.streaming_vm_size
  node_count            = var.streaming_node_count
  os_disk_size_gb       = 100
  vnet_subnet_id        = azurerm_subnet.aks.id
  enable_auto_scaling   = true
  min_count             = var.streaming_min_nodes
  max_count             = var.streaming_max_nodes

  node_labels = {
    workload = "streaming"
  }

  node_taints = ["workload=streaming:NoSchedule"]

  tags = local.common_tags
}

# Azure Cache for Redis
resource "azurerm_redis_cache" "this" {
  count = var.enable_redis ? 1 : 0

  name                = "${var.project_name}-${var.environment}-redis"
  location            = azurerm_resource_group.this.location
  resource_group_name = azurerm_resource_group.this.name
  capacity            = var.redis_capacity
  family              = var.redis_family
  sku_name            = var.redis_sku
  enable_non_ssl_port = false
  minimum_tls_version = "1.2"

  redis_configuration {
    maxmemory_policy = "volatile-lru"
  }

  tags = local.common_tags
}

# Private DNS Zone for PostgreSQL
resource "azurerm_private_dns_zone" "postgres" {
  count = var.enable_postgres ? 1 : 0

  name                = "${var.project_name}-${var.environment}.postgres.database.azure.com"
  resource_group_name = azurerm_resource_group.this.name

  tags = local.common_tags
}

resource "azurerm_private_dns_zone_virtual_network_link" "postgres" {
  count = var.enable_postgres ? 1 : 0

  name                  = "${var.project_name}-${var.environment}-postgres-link"
  resource_group_name   = azurerm_resource_group.this.name
  private_dns_zone_name = azurerm_private_dns_zone.postgres[0].name
  virtual_network_id    = azurerm_virtual_network.this.id

  tags = local.common_tags
}

# Azure Database for PostgreSQL Flexible Server
resource "azurerm_postgresql_flexible_server" "this" {
  count = var.enable_postgres ? 1 : 0

  name                   = "${var.project_name}-${var.environment}-postgres"
  resource_group_name    = azurerm_resource_group.this.name
  location               = azurerm_resource_group.this.location
  version                = "15"
  delegated_subnet_id    = azurerm_subnet.services.id
  private_dns_zone_id    = azurerm_private_dns_zone.postgres[0].id
  administrator_login    = var.postgres_username
  administrator_password = var.postgres_password
  zone                   = "1"
  storage_mb             = var.postgres_storage_mb
  sku_name               = var.postgres_sku

  high_availability {
    mode = var.environment == "production" ? "ZoneRedundant" : "Disabled"
  }

  tags = local.common_tags

  depends_on = [azurerm_private_dns_zone_virtual_network_link.postgres]
}

resource "azurerm_postgresql_flexible_server_database" "dashflow" {
  count = var.enable_postgres ? 1 : 0

  name      = "dashflow"
  server_id = azurerm_postgresql_flexible_server.this[0].id
  charset   = "UTF8"
  collation = "en_US.utf8"
}

# Configure Kubernetes provider with AKS credentials
provider "kubernetes" {
  host                   = azurerm_kubernetes_cluster.this.kube_config[0].host
  client_certificate     = base64decode(azurerm_kubernetes_cluster.this.kube_config[0].client_certificate)
  client_key             = base64decode(azurerm_kubernetes_cluster.this.kube_config[0].client_key)
  cluster_ca_certificate = base64decode(azurerm_kubernetes_cluster.this.kube_config[0].cluster_ca_certificate)
}

# Configure Helm provider
provider "helm" {
  kubernetes {
    host                   = azurerm_kubernetes_cluster.this.kube_config[0].host
    client_certificate     = base64decode(azurerm_kubernetes_cluster.this.kube_config[0].client_certificate)
    client_key             = base64decode(azurerm_kubernetes_cluster.this.kube_config[0].client_key)
    cluster_ca_certificate = base64decode(azurerm_kubernetes_cluster.this.kube_config[0].cluster_ca_certificate)
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
      redis_host    = var.enable_redis ? azurerm_redis_cache.this[0].hostname : ""
      redis_key     = var.enable_redis ? azurerm_redis_cache.this[0].primary_access_key : ""
      postgres_host = var.enable_postgres ? azurerm_postgresql_flexible_server.this[0].fqdn : ""
      azure_region  = var.location
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
    azurerm_kubernetes_cluster.this,
    azurerm_kubernetes_cluster_node_pool.streaming,
    azurerm_redis_cache.this,
    azurerm_postgresql_flexible_server.this
  ]
}

# Local values
locals {
  common_tags = {
    Project     = var.project_name
    Environment = var.environment
    ManagedBy   = "Terraform"
  }
}
