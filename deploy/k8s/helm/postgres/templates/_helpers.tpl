{{/*
Expand the name of the chart.
*/}}
{{- define "angzarr-db-postgres.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
*/}}
{{- define "angzarr-db-postgres.fullname" -}}
{{- .Values.name | default "angzarr-db" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "angzarr-db-postgres.labels" -}}
helm.sh/chart: {{ include "angzarr-db-postgres.chart" . }}
{{ include "angzarr-db-postgres.selectorLabels" . }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels
*/}}
{{- define "angzarr-db-postgres.selectorLabels" -}}
app.kubernetes.io/name: {{ include "angzarr-db-postgres.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Create chart name and version as used by the chart label.
*/}}
{{- define "angzarr-db-postgres.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Secret name for credentials
*/}}
{{- define "angzarr-db-postgres.secretName" -}}
{{- if .Values.auth.existingSecret }}
{{- .Values.auth.existingSecret }}
{{- else }}
{{- include "angzarr-db-postgres.fullname" . }}-credentials
{{- end }}
{{- end }}
