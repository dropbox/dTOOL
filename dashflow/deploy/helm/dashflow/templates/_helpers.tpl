{{/*
Expand the name of the chart.
*/}}
{{- define "dashflow.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
*/}}
{{- define "dashflow.fullname" -}}
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
{{- define "dashflow.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "dashflow.labels" -}}
helm.sh/chart: {{ include "dashflow.chart" . }}
{{ include "dashflow.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels
*/}}
{{- define "dashflow.selectorLabels" -}}
app.kubernetes.io/name: {{ include "dashflow.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Create the name of the service account to use
*/}}
{{- define "dashflow.serviceAccountName" -}}
{{- if .Values.serviceAccount.create }}
{{- default (include "dashflow.fullname" .) .Values.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.serviceAccount.name }}
{{- end }}
{{- end }}

{{/*
Kafka brokers URL
*/}}
{{- define "dashflow.kafkaBrokers" -}}
{{- if .Values.kafka.external }}
{{- .Values.kafka.brokers }}
{{- else }}
{{- printf "%s-kafka:9092" (include "dashflow.fullname" .) }}
{{- end }}
{{- end }}

{{/*
Redis URL
*/}}
{{- define "dashflow.redisUrl" -}}
{{- if .Values.redis.external }}
{{- printf "redis://%s:%d" .Values.redis.host (.Values.redis.port | int) }}
{{- else }}
{{- printf "redis://%s-redis:6379" (include "dashflow.fullname" .) }}
{{- end }}
{{- end }}

{{/*
Jaeger OTLP endpoint
*/}}
{{- define "dashflow.jaegerEndpoint" -}}
{{- printf "http://%s-jaeger:4317" (include "dashflow.fullname" .) }}
{{- end }}

{{/*
Image pull secrets
*/}}
{{- define "dashflow.imagePullSecrets" -}}
{{- with .Values.global.imagePullSecrets }}
imagePullSecrets:
{{- toYaml . | nindent 2 }}
{{- end }}
{{- end }}
