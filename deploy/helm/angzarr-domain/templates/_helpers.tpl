{{/*
Expand the name of the chart.
*/}}
{{- define "angzarr-domain.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
*/}}
{{- define "angzarr-domain.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- printf "%s-%s" .Values.domain .Values.componentType | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}

{{/*
Create chart name and version as used by the chart label.
*/}}
{{- define "angzarr-domain.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "angzarr-domain.labels" -}}
helm.sh/chart: {{ include "angzarr-domain.chart" . }}
{{ include "angzarr-domain.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- range $key, $value := .Values.labels }}
{{ $key }}: {{ $value | quote }}
{{- end }}
{{- end }}

{{/*
Selector labels
*/}}
{{- define "angzarr-domain.selectorLabels" -}}
app.kubernetes.io/name: {{ include "angzarr-domain.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
angzarr.io/domain: {{ .Values.domain }}
angzarr.io/component: {{ .Values.componentType }}
{{- end }}

{{/*
Create the name of the service account to use
*/}}
{{- define "angzarr-domain.serviceAccountName" -}}
{{- if .Values.serviceAccount.create }}
{{- default (include "angzarr-domain.fullname" .) .Values.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.serviceAccount.name }}
{{- end }}
{{- end }}

{{/*
Container security context
*/}}
{{- define "angzarr-domain.containerSecurityContext" -}}
runAsNonRoot: {{ .Values.securityContext.runAsNonRoot }}
runAsUser: {{ .Values.securityContext.runAsUser }}
readOnlyRootFilesystem: {{ .Values.securityContext.readOnlyRootFilesystem }}
allowPrivilegeEscalation: {{ .Values.securityContext.allowPrivilegeEscalation }}
capabilities:
  drop:
    {{- range .Values.securityContext.capabilities.drop }}
    - {{ . }}
    {{- end }}
{{- end }}

{{/*
Logic container port based on component type
*/}}
{{- define "angzarr-domain.logicPort" -}}
{{- if eq .Values.componentType "aggregate" }}50051
{{- else if eq .Values.componentType "saga" }}50052
{{- else if eq .Values.componentType "projector" }}50053
{{- else }}50051
{{- end }}
{{- end }}
