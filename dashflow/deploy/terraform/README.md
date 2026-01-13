# DashFlow Terraform Deployment

This directory contains Terraform configurations for deploying DashFlow to major cloud providers.

## Supported Cloud Providers

| Provider | Directory | Managed Services |
|----------|-----------|------------------|
| AWS | `aws/` | EKS, ElastiCache, MSK, RDS |
| GCP | `gcp/` | GKE, Memorystore, Cloud SQL |
| Azure | `azure/` | AKS, Azure Cache, PostgreSQL Flexible |

## Prerequisites

### All Providers

- Terraform >= 1.5.0
- kubectl
- helm >= 3.0

### AWS

```bash
# Install AWS CLI
curl "https://awscli.amazonaws.com/awscli-exe-linux-x86_64.zip" -o "awscliv2.zip"
unzip awscliv2.zip && sudo ./aws/install

# Configure credentials
aws configure
```

### GCP

```bash
# Install gcloud CLI
curl https://sdk.cloud.google.com | bash

# Authenticate
gcloud auth login
gcloud auth application-default login
```

### Azure

```bash
# Install Azure CLI
curl -sL https://aka.ms/InstallAzureCLIDeb | sudo bash

# Authenticate
az login
```

## Quick Start

### 1. Choose your cloud provider

```bash
cd deploy/terraform/aws    # or gcp, azure
```

### 2. Configure variables

```bash
cp terraform.tfvars.example terraform.tfvars
# Edit terraform.tfvars with your values
```

### 3. Initialize Terraform

```bash
terraform init
```

### 4. Plan deployment

```bash
terraform plan -out=tfplan
```

### 5. Apply

```bash
terraform apply tfplan
```

### 6. Configure kubectl

```bash
# AWS
aws eks update-kubeconfig --name dashflow-dev --region us-west-2

# GCP
gcloud container clusters get-credentials dashflow-dev --region us-west1

# Azure
az aks get-credentials --resource-group dashflow-dev-rg --name dashflow-dev
```

### 7. Verify deployment

```bash
kubectl get pods -n dashflow
kubectl get svc -n dashflow
```

## Environment Configuration

Each cloud provider supports three environments:

| Environment | Replicas | HA | Auto-scaling |
|-------------|----------|-----|--------------|
| dev | 1 | No | No |
| staging | 2 | Partial | Limited |
| production | 3+ | Yes | Full |

Set the environment in your `terraform.tfvars`:

```hcl
environment = "production"
```

## Architecture

### AWS

```
┌─────────────────────────────────────────────────────────────┐
│                          AWS VPC                             │
│  ┌────────────────────┐  ┌────────────────────────────────┐ │
│  │   Public Subnets   │  │      Private Subnets           │ │
│  │  ┌──────────────┐  │  │  ┌─────────┐  ┌─────────────┐  │ │
│  │  │ NAT Gateway  │  │  │  │   EKS   │  │ ElastiCache │  │ │
│  │  └──────────────┘  │  │  │ Cluster │  │   (Redis)   │  │ │
│  │  ┌──────────────┐  │  │  └─────────┘  └─────────────┘  │ │
│  │  │     ALB      │  │  │  ┌─────────┐  ┌─────────────┐  │ │
│  │  └──────────────┘  │  │  │   MSK   │  │     RDS     │  │ │
│  │                    │  │  │ (Kafka) │  │ (PostgreSQL)│  │ │
│  └────────────────────┘  │  └─────────┘  └─────────────┘  │ │
│                          └────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

### GCP

```
┌─────────────────────────────────────────────────────────────┐
│                       GCP VPC Network                        │
│  ┌──────────────────────────────────────────────────────┐   │
│  │                     Subnet                            │   │
│  │  ┌─────────────────┐  ┌─────────────────────────┐    │   │
│  │  │   GKE Cluster   │  │  Memorystore (Redis)    │    │   │
│  │  │  ┌───────────┐  │  └─────────────────────────┘    │   │
│  │  │  │  Node     │  │  ┌─────────────────────────┐    │   │
│  │  │  │  Pools    │  │  │  Cloud SQL (PostgreSQL) │    │   │
│  │  │  └───────────┘  │  └─────────────────────────┘    │   │
│  │  └─────────────────┘                                  │   │
│  └──────────────────────────────────────────────────────┘   │
│  ┌──────────────────┐                                        │
│  │   Cloud NAT      │                                        │
│  └──────────────────┘                                        │
└─────────────────────────────────────────────────────────────┘
```

### Azure

```
┌─────────────────────────────────────────────────────────────┐
│                   Azure Resource Group                       │
│  ┌──────────────────────────────────────────────────────┐   │
│  │                      VNet                             │   │
│  │  ┌─────────────────┐  ┌─────────────────────────┐    │   │
│  │  │   AKS Cluster   │  │  Azure Cache (Redis)    │    │   │
│  │  │  ┌───────────┐  │  └─────────────────────────┘    │   │
│  │  │  │  Node     │  │  ┌─────────────────────────┐    │   │
│  │  │  │  Pools    │  │  │  PostgreSQL Flexible    │    │   │
│  │  │  └───────────┘  │  └─────────────────────────┘    │   │
│  │  └─────────────────┘                                  │   │
│  └──────────────────────────────────────────────────────┘   │
│  ┌──────────────────┐                                        │
│  │ Log Analytics    │                                        │
│  └──────────────────┘                                        │
└─────────────────────────────────────────────────────────────┘
```

## Reusable Modules (AWS)

The `modules/` directory contains reusable Terraform modules:

| Module | Description |
|--------|-------------|
| `vpc` | VPC with public/private subnets, NAT |
| `eks` | EKS cluster with node groups, OIDC |
| `elasticache` | Redis cluster with encryption |
| `msk` | Kafka cluster with TLS |
| `rds` | PostgreSQL with backups |
| `observability` | CloudWatch, X-Ray integration |

## Cost Estimation

### Development (Single Region)

| Provider | ~Monthly Cost |
|----------|---------------|
| AWS | $300-500 |
| GCP | $250-450 |
| Azure | $300-500 |

### Production (HA, Multi-AZ)

| Provider | ~Monthly Cost |
|----------|---------------|
| AWS | $1,500-2,500 |
| GCP | $1,200-2,000 |
| Azure | $1,500-2,500 |

*Estimates vary by region and actual usage.*

## Security Best Practices

1. **Secrets Management**
   - Use AWS Secrets Manager / GCP Secret Manager / Azure Key Vault
   - Never commit `terraform.tfvars` with passwords

2. **Network Security**
   - Private clusters with private endpoints
   - Network policies enabled
   - TLS encryption in transit

3. **Access Control**
   - RBAC configured for Kubernetes
   - Least privilege IAM roles
   - Workload Identity (GCP) / IRSA (AWS)

4. **Encryption**
   - Encryption at rest enabled
   - TLS for all service communication

## Cleanup

To destroy all resources:

```bash
terraform destroy
```

**Warning**: This will delete all data. Ensure backups before destroying production.

## Troubleshooting

### Common Issues

**EKS/GKE/AKS cluster not ready**
```bash
# Check cluster status
kubectl cluster-info
kubectl get nodes
```

**Helm release failed**
```bash
# Check Helm releases
helm list -n dashflow

# Check pod logs
kubectl logs -n dashflow -l app.kubernetes.io/name=dashflow
```

**Network connectivity issues**
```bash
# Test Redis connectivity
kubectl run redis-test --rm -it --image=redis -- redis-cli -h <redis-host> ping

# Test PostgreSQL connectivity
kubectl run pg-test --rm -it --image=postgres -- psql -h <postgres-host> -U dashflow_admin -d dashflow
```

## Support

- GitHub Issues: https://github.com/dropbox/dTOOL/dashflow/issues
- Documentation: https://dashflow.dev/docs
