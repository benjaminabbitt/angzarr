{{/*
Expand the name of the chart.
*/}}
{{- define "postgres-simple.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
For postgres-simple, we want the service to be named angzarr-db-rw
to match what applications expect from CloudNativePG.
*/}}
{{- define "postgres-simple.fullname" -}}
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
Service name - just the release name for simple lookup
Apps expect service at "postgres" when release is "postgres"
*/}}
{{- define "postgres-simple.serviceName" -}}
{{- .Release.Name }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "postgres-simple.labels" -}}
helm.sh/chart: {{ include "postgres-simple.name" . }}
{{ include "postgres-simple.selectorLabels" . }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels
*/}}
{{- define "postgres-simple.selectorLabels" -}}
app.kubernetes.io/name: {{ include "postgres-simple.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}
