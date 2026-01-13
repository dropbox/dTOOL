# DashFlow AWS Infrastructure
# Terraform configuration for deploying DashFlow on AWS EKS

terraform {
  required_version = ">= 1.5.0"

  required_providers {
    aws = {
      source  = "hashicorp/aws"
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
  # backend "s3" {
  #   bucket         = "dashflow-terraform-state"
  #   key            = "aws/terraform.tfstate"
  #   region         = "us-west-2"
  #   encrypt        = true
  #   dynamodb_table = "dashflow-terraform-locks"
  # }
}

provider "aws" {
  region = var.aws_region

  default_tags {
    tags = {
      Project     = "DashFlow"
      Environment = var.environment
      ManagedBy   = "Terraform"
    }
  }
}

# Data sources for existing AWS resources
data "aws_availability_zones" "available" {
  state = "available"
}

data "aws_caller_identity" "current" {}

# VPC Module
module "vpc" {
  source = "../modules/vpc"

  name               = "${var.project_name}-${var.environment}"
  cidr               = var.vpc_cidr
  availability_zones = slice(data.aws_availability_zones.available.names, 0, 3)

  enable_nat_gateway     = var.enable_nat_gateway
  single_nat_gateway     = var.environment != "production"
  enable_dns_hostnames   = true
  enable_dns_support     = true

  tags = local.common_tags
}

# EKS Cluster Module
module "eks" {
  source = "../modules/eks"

  cluster_name    = "${var.project_name}-${var.environment}"
  cluster_version = var.eks_cluster_version

  vpc_id          = module.vpc.vpc_id
  subnet_ids      = module.vpc.private_subnet_ids

  node_groups = var.eks_node_groups

  # Cluster addons
  enable_cluster_autoscaler = true
  enable_metrics_server     = true
  enable_aws_load_balancer_controller = true

  tags = local.common_tags

  depends_on = [module.vpc]
}

# ElastiCache (Redis) for caching and replay buffer
module "elasticache" {
  source = "../modules/elasticache"
  count  = var.enable_redis ? 1 : 0

  cluster_id           = "${var.project_name}-${var.environment}-redis"
  node_type            = var.redis_node_type
  num_cache_nodes      = var.environment == "production" ? 3 : 1
  parameter_group_name = "default.redis7"
  engine_version       = "7.0"
  port                 = 6379

  vpc_id     = module.vpc.vpc_id
  subnet_ids = module.vpc.private_subnet_ids

  security_group_ids = [module.eks.cluster_security_group_id]

  tags = local.common_tags

  depends_on = [module.vpc]
}

# MSK (Managed Kafka) for streaming
module "msk" {
  source = "../modules/msk"
  count  = var.enable_kafka ? 1 : 0

  cluster_name           = "${var.project_name}-${var.environment}"
  kafka_version          = var.kafka_version
  number_of_broker_nodes = var.environment == "production" ? 3 : 2
  broker_instance_type   = var.kafka_broker_instance_type

  vpc_id     = module.vpc.vpc_id
  subnet_ids = module.vpc.private_subnet_ids

  security_group_ids = [module.eks.cluster_security_group_id]

  # Encryption
  encryption_in_transit_client_broker = "TLS"
  encryption_in_transit_in_cluster    = true

  # Monitoring
  enhanced_monitoring = var.environment == "production" ? "PER_TOPIC_PER_BROKER" : "DEFAULT"

  tags = local.common_tags

  depends_on = [module.vpc]
}

# RDS (PostgreSQL) for persistent checkpointing
module "rds" {
  source = "../modules/rds"
  count  = var.enable_postgres ? 1 : 0

  identifier = "${var.project_name}-${var.environment}"

  engine            = "postgres"
  engine_version    = "15.4"
  instance_class    = var.rds_instance_class
  allocated_storage = var.rds_allocated_storage
  storage_encrypted = true

  db_name  = "dashflow"
  username = var.rds_username
  password = var.rds_password

  vpc_id     = module.vpc.vpc_id
  subnet_ids = module.vpc.private_subnet_ids

  security_group_ids = [module.eks.cluster_security_group_id]

  # High availability
  multi_az = var.environment == "production"

  # Backup
  backup_retention_period = var.environment == "production" ? 30 : 7
  backup_window           = "03:00-04:00"
  maintenance_window      = "sun:04:00-sun:05:00"

  # Performance Insights
  performance_insights_enabled = var.environment == "production"

  tags = local.common_tags

  depends_on = [module.vpc]
}

# Observability Module (CloudWatch, X-Ray)
module "observability" {
  source = "../modules/observability"

  project_name = var.project_name
  environment  = var.environment

  eks_cluster_name = module.eks.cluster_name
  eks_oidc_provider_arn = module.eks.oidc_provider_arn

  # CloudWatch
  enable_container_insights = true
  log_retention_days        = var.environment == "production" ? 90 : 30

  # X-Ray
  enable_xray = var.enable_xray

  tags = local.common_tags

  depends_on = [module.eks]
}

# Configure Kubernetes provider with EKS credentials
provider "kubernetes" {
  host                   = module.eks.cluster_endpoint
  cluster_ca_certificate = base64decode(module.eks.cluster_ca_certificate)

  exec {
    api_version = "client.authentication.k8s.io/v1beta1"
    command     = "aws"
    args        = ["eks", "get-token", "--cluster-name", module.eks.cluster_name]
  }
}

# Configure Helm provider
provider "helm" {
  kubernetes {
    host                   = module.eks.cluster_endpoint
    cluster_ca_certificate = base64decode(module.eks.cluster_ca_certificate)

    exec {
      api_version = "client.authentication.k8s.io/v1beta1"
      command     = "aws"
      args        = ["eks", "get-token", "--cluster-name", module.eks.cluster_name]
    }
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
      redis_endpoint    = var.enable_redis ? module.elasticache[0].endpoint : ""
      kafka_brokers     = var.enable_kafka ? module.msk[0].bootstrap_brokers_tls : ""
      postgres_endpoint = var.enable_postgres ? module.rds[0].endpoint : ""
      aws_region        = var.aws_region
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
    module.eks,
    module.elasticache,
    module.msk,
    module.rds
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
