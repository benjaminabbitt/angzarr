{{/*
Common labels
*/}}
{{- define "observability.labels" -}}
helm.sh/chart: {{ .Chart.Name }}-{{ .Chart.Version | replace "+" "_" }}
app.kubernetes.io/name: {{ .Chart.Name }}
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
OTel Collector endpoint (for use in Grafana datasource config)
*/}}
{{- define "observability.otelCollectorEndpoint" -}}
{{ .Release.Name }}-opentelemetry-collector.{{ .Release.Namespace }}.svc.cluster.local:4317
{{- end }}

{{/*
Tempo endpoint
*/}}
{{- define "observability.tempoEndpoint" -}}
http://{{ .Release.Name }}-tempo.{{ .Release.Namespace }}.svc.cluster.local:3100
{{- end }}

{{/*
Prometheus endpoint
*/}}
{{- define "observability.prometheusEndpoint" -}}
http://{{ .Release.Name }}-prometheus-server.{{ .Release.Namespace }}.svc.cluster.local:80
{{- end }}

{{/*
Loki endpoint
*/}}
{{- define "observability.lokiEndpoint" -}}
http://{{ .Release.Name }}-loki.{{ .Release.Namespace }}.svc.cluster.local:3100
{{- end }}
