# ElastiCache Module Outputs

output "endpoint" {
  description = "ElastiCache primary endpoint"
  value       = aws_elasticache_replication_group.this.primary_endpoint_address
}

output "reader_endpoint" {
  description = "ElastiCache reader endpoint"
  value       = aws_elasticache_replication_group.this.reader_endpoint_address
}

output "port" {
  description = "ElastiCache port"
  value       = var.port
}

output "security_group_id" {
  description = "Security group ID"
  value       = aws_security_group.this.id
}
