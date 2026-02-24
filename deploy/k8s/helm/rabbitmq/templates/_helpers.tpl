{{/*
Expand the name of the chart.
*/}}
{{- define "angzarr-mq-rabbitmq.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
*/}}
{{- define "angzarr-mq-rabbitmq.fullname" -}}
{{- .Values.name | default "angzarr-mq" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "angzarr-mq-rabbitmq.labels" -}}
helm.sh/chart: {{ include "angzarr-mq-rabbitmq.chart" . }}
{{ include "angzarr-mq-rabbitmq.selectorLabels" . }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels
*/}}
{{- define "angzarr-mq-rabbitmq.selectorLabels" -}}
app.kubernetes.io/name: {{ include "angzarr-mq-rabbitmq.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Create chart name and version as used by the chart label.
*/}}
{{- define "angzarr-mq-rabbitmq.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Secret name for credentials
*/}}
{{- define "angzarr-mq-rabbitmq.secretName" -}}
{{- if .Values.auth.existingSecret }}
{{- .Values.auth.existingSecret }}
{{- else }}
{{- include "angzarr-mq-rabbitmq.fullname" . }}-credentials
{{- end }}
{{- end }}
