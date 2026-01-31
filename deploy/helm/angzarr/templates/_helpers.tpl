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
*/}}
{{- define "angzarr.topology-enabled" -}}
{{- or .Values.infrastructure.topology.enabled .Values.observability.enabled -}}
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
