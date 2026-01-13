# Observability Module Outputs

output "cloudwatch_log_group_name" {
  description = "CloudWatch log group name"
  value       = aws_cloudwatch_log_group.application.name
}

output "container_insights_role_arn" {
  description = "Container Insights IAM role ARN"
  value       = var.enable_container_insights ? aws_iam_role.container_insights[0].arn : null
}

output "xray_role_arn" {
  description = "X-Ray IAM role ARN"
  value       = var.enable_xray ? aws_iam_role.xray[0].arn : null
}

output "dashboard_name" {
  description = "CloudWatch dashboard name"
  value       = aws_cloudwatch_dashboard.this.dashboard_name
}
