{{/* ---- names and labels ------------------------------------------------------------------ */}}
{{- define "notturni.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "notturni.fullname" -}}
{{- if .Values.fullnameOverride -}}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" -}}
{{- else -}}
{{- $name := default .Chart.Name .Values.nameOverride -}}
{{- if contains $name .Release.Name -}}
{{- .Release.Name | trunc 63 | trimSuffix "-" -}}
{{- else -}}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" -}}
{{- end -}}
{{- end -}}
{{- end -}}

{{/* Resource name of a component: usage {{ include "notturni.cname" (list . "backend") }} */}}
{{- define "notturni.cname" -}}
{{- $root := index . 0 -}}
{{- printf "%s-%s" (include "notturni.fullname" $root) (index . 1) | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "notturni.labels" -}}
helm.sh/chart: {{ printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
app.kubernetes.io/name: {{ include "notturni.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end -}}

{{- define "notturni.selectorLabels" -}}
app.kubernetes.io/name: {{ include "notturni.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end -}}

{{/* Labels of a component: usage {{ include "notturni.componentLabels" (list . "backend") }} */}}
{{- define "notturni.componentLabels" -}}
{{ include "notturni.labels" (index . 0) }}
app.kubernetes.io/component: {{ index . 1 }}
{{- end -}}

{{- define "notturni.componentSelector" -}}
{{ include "notturni.selectorLabels" (index . 0) }}
app.kubernetes.io/component: {{ index . 1 }}
{{- end -}}

{{/* Image reference: usage {{ include "notturni.image" (list . .Values.image.backend) }} (empty tag = appVersion) */}}
{{- define "notturni.image" -}}
{{- $root := index . 0 -}}
{{- $img := index . 1 -}}
{{- printf "%s:%s" $img.repository (default $root.Chart.AppVersion $img.tag) -}}
{{- end -}}

{{- define "notturni.imagePullSecrets" -}}
{{- with .Values.image.imagePullSecrets }}
imagePullSecrets:
  {{- toYaml . | nindent 2 }}
{{- end }}
{{- end -}}

{{/* ---- derived switches ------------------------------------------------------------------ */}}
{{- define "notturni.isLocal" -}}{{ if eq .Values.storage.type "local" }}true{{ end }}{{- end -}}
{{- define "notturni.garage" -}}{{ if and (eq .Values.storage.type "s3") .Values.storage.s3.garage.enabled }}true{{ end }}{{- end -}}
{{- define "notturni.internalPostgres" -}}{{ if not .Values.postgres.external.host }}true{{ end }}{{- end -}}
{{- define "notturni.scheme" -}}{{ if .Values.ingress.tls.enabled }}https{{ else }}http{{ end }}{{- end -}}
{{- define "notturni.baseUrl" -}}{{ printf "%s://%s" (include "notturni.scheme" .) (required "ingress.host is required" .Values.ingress.host) }}{{- end -}}

{{- define "notturni.secretName" -}}
{{- default (include "notturni.cname" (list . "secrets")) .Values.secrets.existingSecret -}}
{{- end -}}

{{- define "notturni.tlsSecretName" -}}
{{- default (include "notturni.cname" (list . "tls")) .Values.ingress.tls.secretName -}}
{{- end -}}

{{- define "notturni.postgresHost" -}}
{{- if .Values.postgres.external.host -}}{{ .Values.postgres.external.host }}{{- else -}}{{ include "notturni.cname" (list . "postgres") }}{{- end -}}
{{- end -}}

{{- define "notturni.s3Host" -}}
{{- default (printf "s3.%s" .Values.ingress.host) .Values.storage.s3.garage.publicHost -}}
{{- end -}}

{{- define "notturni.storagePublicUrl" -}}
{{- default (printf "%s/storage" (include "notturni.baseUrl" .)) .Values.storage.local.publicUrl -}}
{{- end -}}

{{/* Module list: `image_moderation` is kept only when moderation.enabled, whatever the user listed.
     An empty list stays empty (NOCT_MODULES omitted = backend default set). */}}
{{- define "notturni.modules" -}}
{{- $list := list -}}
{{- range .Values.modules -}}
{{- if ne . "image_moderation" -}}{{- $list = append $list . -}}{{- end -}}
{{- end -}}
{{- if and .Values.moderation.enabled (gt (len .Values.modules) 0) -}}{{- $list = append $list "image_moderation" -}}{{- end -}}
{{- join "," $list -}}
{{- end -}}

{{/* ---- consistency rules ----------------------------------------------------------------- */}}
{{- define "notturni.validate" -}}
{{- if not (has .Values.mode (list "solo" "platform")) -}}{{- fail "mode must be `solo` or `platform`" -}}{{- end -}}
{{- if not (has .Values.storage.type (list "local" "s3")) -}}{{- fail "storage.type must be `local` or `s3`" -}}{{- end -}}
{{- if not .Values.ingress.host -}}{{- fail "ingress.host is required (public host name, e.g. example.com)" -}}{{- end -}}
{{- if and .Values.subdomains.enabled (ne .Values.mode "platform") -}}
{{- fail "subdomains.enabled=true needs mode=platform (solo serves blogs on /<slug> paths only)" -}}
{{- end -}}
{{- if and (include "notturni.isLocal" .) (gt (int .Values.backend.replicaCount) 1) -}}
{{- fail "storage.type=local keeps objects on a ReadWriteOnce volume: backend.replicaCount must be 1 (use storage.type=s3 to scale)" -}}
{{- end -}}
{{- if and (not .Values.nats.enabled) (gt (int .Values.backend.replicaCount) 1) -}}
{{- fail "nats.enabled=false uses the in-process bus and the inline worker: backend.replicaCount must be 1 (enable nats to scale)" -}}
{{- end -}}
{{- if eq .Values.storage.type "s3" -}}
{{- if and (not .Values.storage.s3.garage.enabled) (not .Values.storage.s3.endpointUrl) -}}
{{- fail "storage.type=s3 needs storage.s3.endpointUrl (external S3) or storage.s3.garage.enabled=true" -}}
{{- end -}}
{{- if and (not .Values.storage.s3.garage.enabled) (not .Values.storage.s3.publicUrl) -}}
{{- fail "storage.type=s3 with external S3 needs storage.s3.publicUrl (public base URL of the objects)" -}}
{{- end -}}
{{- end -}}
{{- if and .Values.storage.s3.garage.enabled (ne .Values.storage.type "s3") -}}
{{- fail "storage.s3.garage.enabled=true needs storage.type=s3" -}}
{{- end -}}
{{- if and .Values.ingress.tls.certManager.enabled (not .Values.ingress.tls.enabled) -}}
{{- fail "ingress.tls.certManager.enabled needs ingress.tls.enabled=true" -}}
{{- end -}}
{{- if and .Values.ingress.tls.certManager.createIssuer.enabled (not .Values.ingress.tls.certManager.createIssuer.email) -}}
{{- fail "ingress.tls.certManager.createIssuer.email is required to create an Issuer" -}}
{{- end -}}
{{- end -}}

{{/* base64 of the value kept from the existing Secret, or of a fresh one: (list $old key fresh) */}}
{{- define "notturni.keep" -}}
{{- $old := index . 0 -}}{{- $key := index . 1 -}}{{- $fresh := index . 2 -}}
{{- if hasKey $old $key -}}{{ index $old $key }}{{- else -}}{{ $fresh | b64enc }}{{- end -}}
{{- end -}}
