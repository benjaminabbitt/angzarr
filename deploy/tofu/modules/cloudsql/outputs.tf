# Cloud SQL Module - Outputs

output "instance_name" {
  description = "Name of the Cloud SQL instance"
  value       = google_sql_database_instance.instance.name
}

output "connection_name" {
  description = "Connection name for Cloud SQL Proxy"
  value       = google_sql_database_instance.instance.connection_name
}

output "private_ip" {
  description = "Private IP address (null if not enabled)"
  value       = local.private_ip
}

output "public_ip" {
  description = "Public IP address (null if not enabled)"
  value       = local.public_ip
}

output "database" {
  description = "Database name"
  value       = google_sql_database.database.name
}

output "username" {
  description = "Database username"
  value       = google_sql_user.user.name
}

output "password" {
  description = "Database password"
  value       = local.password
  sensitive   = true
}

# Connection URIs
output "proxy_uri" {
  description = "Connection URI for Cloud SQL Proxy (use with Cloud Run)"
  value       = local.proxy_uri
  sensitive   = true
}

output "private_uri" {
  description = "Connection URI for private IP (requires VPC connector)"
  value       = local.private_uri
  sensitive   = true
}

output "public_uri" {
  description = "Connection URI for public IP"
  value       = local.public_uri
  sensitive   = true
}

# Secret references (for Cloud Run env vars)
output "password_secret_id" {
  description = "Secret Manager secret ID for password"
  value       = var.create_secrets ? google_secret_manager_secret.db_password[0].secret_id : null
}

output "uri_secret_id" {
  description = "Secret Manager secret ID for connection URI"
  value       = var.create_secrets ? google_secret_manager_secret.db_uri[0].secret_id : null
}

output "password_secret_version" {
  description = "Secret Manager secret version for password"
  value       = var.create_secrets ? google_secret_manager_secret_version.db_password[0].version : null
}

output "uri_secret_version" {
  description = "Secret Manager secret version for URI"
  value       = var.create_secrets ? google_secret_manager_secret_version.db_uri[0].version : null
}

# Cloud Run secret reference format
output "cloudrun_secret_ref" {
  description = "Secret reference for Cloud Run DATABASE_URL env var"
  value = var.create_secrets ? {
    secret  = google_secret_manager_secret.db_uri[0].secret_id
    version = "latest"
  } : null
}
