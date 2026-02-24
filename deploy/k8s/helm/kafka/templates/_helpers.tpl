{{/*
Expand the name of the chart.
*/}}
{{- define "angzarr-mq-kafka.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
*/}}
{{- define "angzarr-mq-kafka.fullname" -}}
{{- .Values.name | default "angzarr-kafka" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "angzarr-mq-kafka.labels" -}}
helm.sh/chart: {{ include "angzarr-mq-kafka.chart" . }}
{{ include "angzarr-mq-kafka.selectorLabels" . }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels
*/}}
{{- define "angzarr-mq-kafka.selectorLabels" -}}
app.kubernetes.io/name: {{ include "angzarr-mq-kafka.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Create chart name and version as used by the chart label.
*/}}
{{- define "angzarr-mq-kafka.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Bootstrap server address
*/}}
{{- define "angzarr-mq-kafka.bootstrapServers" -}}
{{- include "angzarr-mq-kafka.fullname" . }}-kafka-bootstrap.{{ .Release.Namespace }}.svc.cluster.local:9092
{{- end }}
