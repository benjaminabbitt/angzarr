{{/*
Expand the name of the chart.
*/}}
{{- define "rabbitmq-simple.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
*/}}
{{- define "rabbitmq-simple.fullname" -}}
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
Service name - just the release name to match expected service naming
Apps expect service at "angzarr-mq" when release is "angzarr-mq"
*/}}
{{- define "rabbitmq-simple.serviceName" -}}
{{- .Release.Name }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "rabbitmq-simple.labels" -}}
helm.sh/chart: {{ include "rabbitmq-simple.name" . }}
{{ include "rabbitmq-simple.selectorLabels" . }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels
*/}}
{{- define "rabbitmq-simple.selectorLabels" -}}
app.kubernetes.io/name: {{ include "rabbitmq-simple.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}
