{{/*
Expand the name of the chart.
*/}}
{{- define "rushdb.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
We truncate at 63 chars because some Kubernetes name fields are limited to this.
*/}}
{{- define "rushdb.fullname" -}}
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
Fully qualified name for bundled Neo4j resources.
*/}}
{{- define "rushdb.neo4j.fullname" -}}
{{- printf "%s-neo4j" (include "rushdb.fullname" .) | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create chart label.
*/}}
{{- define "rushdb.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels — RushDB platform.
*/}}
{{- define "rushdb.labels" -}}
helm.sh/chart: {{ include "rushdb.chart" . }}
{{ include "rushdb.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels — RushDB platform.
*/}}
{{- define "rushdb.selectorLabels" -}}
app.kubernetes.io/name: {{ include "rushdb.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Common labels — bundled Neo4j.
*/}}
{{- define "rushdb.neo4j.labels" -}}
helm.sh/chart: {{ include "rushdb.chart" . }}
{{ include "rushdb.neo4j.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels — bundled Neo4j.
*/}}
{{- define "rushdb.neo4j.selectorLabels" -}}
app.kubernetes.io/name: {{ include "rushdb.neo4j.fullname" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Resolve the Neo4j bolt URL for RushDB.
When the bundled Neo4j is enabled, auto-wire the service DNS.
Otherwise, use the user-supplied URL.
*/}}
{{- define "rushdb.neo4jUrl" -}}
{{- if .Values.neo4j.enabled }}
{{- printf "bolt://%s:%d" (include "rushdb.neo4j.fullname" .) (int .Values.neo4j.service.boltPort) }}
{{- else }}
{{- required "rushdb.neo4jUrl is required when neo4j.enabled=false" .Values.rushdb.neo4jUrl }}
{{- end }}
{{- end }}

{{/*
Resolve the Neo4j password for RushDB.
*/}}
{{- define "rushdb.neo4jPassword" -}}
{{- if .Values.neo4j.enabled }}
{{- .Values.neo4j.password }}
{{- else }}
{{- .Values.rushdb.neo4jPassword }}
{{- end }}
{{- end }}

{{/*
Resolve the Neo4j username for RushDB.
*/}}
{{- define "rushdb.neo4jUsername" -}}
{{- if .Values.neo4j.enabled -}}
neo4j
{{- else -}}
{{- default "neo4j" .Values.rushdb.neo4jUsername }}
{{- end -}}
{{- end }}
