# Observability Module for DashFlow

# CloudWatch Log Group for EKS
resource "aws_cloudwatch_log_group" "eks" {
  name              = "/aws/eks/${var.eks_cluster_name}/cluster"
  retention_in_days = var.log_retention_days

  tags = var.tags
}

# CloudWatch Log Group for Application
resource "aws_cloudwatch_log_group" "application" {
  name              = "/aws/containerinsights/${var.eks_cluster_name}/application"
  retention_in_days = var.log_retention_days

  tags = var.tags
}

# IAM Role for Container Insights
resource "aws_iam_role" "container_insights" {
  count = var.enable_container_insights ? 1 : 0
  name  = "${var.project_name}-${var.environment}-container-insights"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Action = "sts:AssumeRoleWithWebIdentity"
      Effect = "Allow"
      Principal = {
        Federated = var.eks_oidc_provider_arn
      }
      Condition = {
        StringEquals = {
          "${replace(var.eks_oidc_provider_arn, "/^.*provider\\//", "")}:sub" = "system:serviceaccount:amazon-cloudwatch:cloudwatch-agent"
        }
      }
    }]
  })

  tags = var.tags
}

resource "aws_iam_role_policy_attachment" "container_insights" {
  count      = var.enable_container_insights ? 1 : 0
  policy_arn = "arn:aws:iam::aws:policy/CloudWatchAgentServerPolicy"
  role       = aws_iam_role.container_insights[0].name
}

# IAM Role for X-Ray
resource "aws_iam_role" "xray" {
  count = var.enable_xray ? 1 : 0
  name  = "${var.project_name}-${var.environment}-xray"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Action = "sts:AssumeRoleWithWebIdentity"
      Effect = "Allow"
      Principal = {
        Federated = var.eks_oidc_provider_arn
      }
      Condition = {
        StringEquals = {
          "${replace(var.eks_oidc_provider_arn, "/^.*provider\\//", "")}:sub" = "system:serviceaccount:${var.project_name}:xray-daemon"
        }
      }
    }]
  })

  tags = var.tags
}

resource "aws_iam_role_policy_attachment" "xray" {
  count      = var.enable_xray ? 1 : 0
  policy_arn = "arn:aws:iam::aws:policy/AWSXRayDaemonWriteAccess"
  role       = aws_iam_role.xray[0].name
}

# CloudWatch Dashboard
resource "aws_cloudwatch_dashboard" "this" {
  dashboard_name = "${var.project_name}-${var.environment}"

  dashboard_body = jsonencode({
    widgets = [
      {
        type   = "metric"
        x      = 0
        y      = 0
        width  = 12
        height = 6
        properties = {
          title  = "CPU Utilization"
          view   = "timeSeries"
          region = data.aws_region.current.name
          metrics = [
            ["ContainerInsights", "pod_cpu_utilization", "ClusterName", var.eks_cluster_name]
          ]
        }
      },
      {
        type   = "metric"
        x      = 12
        y      = 0
        width  = 12
        height = 6
        properties = {
          title  = "Memory Utilization"
          view   = "timeSeries"
          region = data.aws_region.current.name
          metrics = [
            ["ContainerInsights", "pod_memory_utilization", "ClusterName", var.eks_cluster_name]
          ]
        }
      },
      {
        type   = "metric"
        x      = 0
        y      = 6
        width  = 12
        height = 6
        properties = {
          title  = "Network In/Out"
          view   = "timeSeries"
          region = data.aws_region.current.name
          metrics = [
            ["ContainerInsights", "pod_network_rx_bytes", "ClusterName", var.eks_cluster_name],
            ["ContainerInsights", "pod_network_tx_bytes", "ClusterName", var.eks_cluster_name]
          ]
        }
      },
      {
        type   = "metric"
        x      = 12
        y      = 6
        width  = 12
        height = 6
        properties = {
          title  = "Pod Count"
          view   = "timeSeries"
          region = data.aws_region.current.name
          metrics = [
            ["ContainerInsights", "pod_number_of_running_pods", "ClusterName", var.eks_cluster_name]
          ]
        }
      }
    ]
  })
}

# CloudWatch Alarms
resource "aws_cloudwatch_metric_alarm" "high_cpu" {
  alarm_name          = "${var.project_name}-${var.environment}-high-cpu"
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = 3
  metric_name         = "pod_cpu_utilization"
  namespace           = "ContainerInsights"
  period              = 300
  statistic           = "Average"
  threshold           = 80
  alarm_description   = "CPU utilization is above 80%"

  dimensions = {
    ClusterName = var.eks_cluster_name
  }

  tags = var.tags
}

resource "aws_cloudwatch_metric_alarm" "high_memory" {
  alarm_name          = "${var.project_name}-${var.environment}-high-memory"
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = 3
  metric_name         = "pod_memory_utilization"
  namespace           = "ContainerInsights"
  period              = 300
  statistic           = "Average"
  threshold           = 80
  alarm_description   = "Memory utilization is above 80%"

  dimensions = {
    ClusterName = var.eks_cluster_name
  }

  tags = var.tags
}

data "aws_region" "current" {}
