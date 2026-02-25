{{/*
Expand the name of the chart.
*/}}
{{- define "angzarr.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
*/}}
{{- define "angzarr.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{/*
Create chart name and version as used by the chart label.
*/}}
{{- define "angzarr.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "angzarr.labels" -}}
helm.sh/chart: {{ include "angzarr.chart" . }}
{{ include "angzarr.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels
*/}}
{{- define "angzarr.selectorLabels" -}}
app.kubernetes.io/name: {{ include "angzarr.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Create the name of the service account to use
*/}}
{{- define "angzarr.serviceAccountName" -}}
{{- if .Values.serviceAccount.create }}
{{- default (include "angzarr.fullname" .) .Values.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.serviceAccount.name }}
{{- end }}
{{- end }}

{{/*
OTel environment variables for angzarr sidecar containers.
Usage: {{- include "angzarr.otel-env" (dict "root" $ "service" "aggregate" "domain" .domain) | nindent 12 }}
*/}}
{{- define "angzarr.otel-env" -}}
{{- if .root.Values.observability.enabled }}
# OpenTelemetry configuration
- name: OTEL_EXPORTER_OTLP_ENDPOINT
  value: {{ .root.Values.observability.otlp.endpoint | quote }}
- name: OTEL_SERVICE_NAME
  {{- if .domain }}
  value: "angzarr-{{ .service }}-{{ .domain }}"
  {{- else }}
  value: "angzarr-{{ .service }}"
  {{- end }}
- name: OTEL_RESOURCE_ATTRIBUTES
  value: {{ printf "deployment.environment=%s,service.namespace=angzarr,service.version=%s" .root.Values.observability.environment (.root.Chart.AppVersion | default "0.1.0") | quote }}
{{- end }}
{{- end }}

{{/*
Whether topology service is enabled.
Auto-enables when observability is enabled (Grafana stack implies topology visualization).
Returns empty string if disabled (falsy in Helm), "true" if enabled.
*/}}
{{- define "angzarr.topology-enabled" -}}
{{- if or .Values.infrastructure.topology.enabled .Values.observability.enabled -}}
true
{{- end -}}
{{- end }}

{{/*
Component naming convention: {domain}-{type}
Per-domain aggregate coordinator names follow the pattern: player-aggregate, order-aggregate
This enables consistent DNS routing: player-aggregate.angzarr.svc.cluster.local:1310
*/}}

{{/*
Aggregate service name: {domain}-aggregate
Usage: {{ include "angzarr.aggregate-service-name" (dict "domain" .domain) }}
*/}}
{{- define "angzarr.aggregate-service-name" -}}
{{- printf "%s-aggregate" .domain }}
{{- end }}

{{/*
Saga service name: {name}-saga
Usage: {{ include "angzarr.saga-service-name" (dict "name" .name) }}
*/}}
{{- define "angzarr.saga-service-name" -}}
{{- printf "%s-saga" .name }}
{{- end }}

{{/*
Projector service name: {name}-projector
Usage: {{ include "angzarr.projector-service-name" (dict "name" .name) }}
*/}}
{{- define "angzarr.projector-service-name" -}}
{{- printf "%s-projector" .name }}
{{- end }}

{{/*
Process manager service name: {name}-pm
Usage: {{ include "angzarr.pm-service-name" (dict "name" .name) }}
*/}}
{{- define "angzarr.pm-service-name" -}}
{{- printf "%s-pm" .name }}
{{- end }}

{{/*
Static endpoints for command routing to aggregates.
Builds a comma-separated list of domain=address pairs.
Usage: {{ include "angzarr.static-endpoints" (dict "root" $ "port" $.Values.service.aggregatePort) }}
*/}}
{{- define "angzarr.static-endpoints" -}}
{{- $endpoints := list -}}
{{- range .root.Values.applications.business -}}
{{- $serviceName := printf "%s-aggregate" .domain -}}
{{- $endpoints = append $endpoints (printf "%s=%s:%d" .domain $serviceName (int $.port)) -}}
{{- end -}}
{{- join "," $endpoints -}}
{{- end }}

{{/*
Container security context with optional debug capabilities
runAsNonRoot is hardcoded - never allow running as root
*/}}
{{- define "angzarr.containerSecurityContext" -}}
runAsNonRoot: true
runAsUser: {{ .Values.securityContext.runAsUser }}
readOnlyRootFilesystem: {{ .Values.securityContext.readOnlyRootFilesystem }}
allowPrivilegeEscalation: false
capabilities:
  drop:
    - ALL
  {{- if .Values.debug.enabled }}
  add:
    - SYS_PTRACE
  {{- end }}
{{- end }}
