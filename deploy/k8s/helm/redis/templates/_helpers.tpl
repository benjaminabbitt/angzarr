{{/*
Expand the name of the chart.
*/}}
{{- define "angzarr-db-redis.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
*/}}
{{- define "angzarr-db-redis.fullname" -}}
{{- .Values.name | default "angzarr-redis" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "angzarr-db-redis.labels" -}}
helm.sh/chart: {{ include "angzarr-db-redis.chart" . }}
{{ include "angzarr-db-redis.selectorLabels" . }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels
*/}}
{{- define "angzarr-db-redis.selectorLabels" -}}
app.kubernetes.io/name: {{ include "angzarr-db-redis.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Create chart name and version as used by the chart label.
*/}}
{{- define "angzarr-db-redis.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Secret name for credentials
*/}}
{{- define "angzarr-db-redis.secretName" -}}
{{- if .Values.auth.existingSecret }}
{{- .Values.auth.existingSecret }}
{{- else }}
{{- include "angzarr-db-redis.fullname" . }}
{{- end }}
{{- end }}
