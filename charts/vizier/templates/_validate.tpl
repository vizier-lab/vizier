{{- define "vizier.validate" -}}
{{- if ne (int .Values.replicaCount) 1 -}}
{{- fail "replicaCount must be 1; multiple Vizier replicas require Fleet support" -}}
{{- end -}}
{{- if ge (int .Values.shutdown.gracePeriodSeconds) (int .Values.terminationGracePeriodSeconds) -}}
{{- fail "shutdown.gracePeriodSeconds must be less than terminationGracePeriodSeconds" -}}
{{- end -}}
{{- if eq .Values.image.tag "latest" -}}
{{- fail "image.tag must be an immutable release tag, not latest" -}}
{{- end -}}
{{- if or (empty .Values.secrets.jwt.name) (empty .Values.secrets.jwt.key) -}}
{{- fail "secrets.jwt.name and key are required; reference a Kubernetes Secret containing a strong JWT secret" -}}
{{- end -}}
{{- if ne (empty .Values.secrets.encryptionKey.name) (empty .Values.secrets.encryptionKey.key) -}}
{{- fail "secrets.encryptionKey.name and key must be set together" -}}
{{- end -}}
{{- if and .Values.ingress.enabled (eq (len .Values.ingress.hosts) 0) -}}
{{- fail "ingress.hosts must contain at least one host when ingress is enabled" -}}
{{- end -}}
{{- if .Values.ingress.enabled -}}
  {{- range $index, $host := .Values.ingress.hosts -}}
    {{- if empty $host.host -}}
    {{- fail (printf "ingress.hosts[%d].host is required" $index) -}}
    {{- end -}}
    {{- if eq (len $host.paths) 0 -}}
    {{- fail (printf "ingress.hosts[%d].paths must contain at least one path" $index) -}}
    {{- end -}}
  {{- end -}}
{{- end -}}
{{- if and .Values.trustedCa.enabled (empty .Values.trustedCa.configMapName) -}}
{{- fail "trustedCa.configMapName is required when trustedCa is enabled" -}}
{{- end -}}
{{- if eq .Values.storage.profile "embedded" -}}
  {{- if not .Values.persistence.enabled -}}
  {{- fail "persistence.enabled must be true for the embedded storage profile" -}}
  {{- end -}}
{{- else if eq .Values.storage.profile "managed" -}}
  {{- if or (empty .Values.managed.database.urlSecretRef.name) (empty .Values.managed.database.urlSecretRef.key) -}}
  {{- fail "managed.database.urlSecretRef.name and key are required for managed storage" -}}
  {{- end -}}
  {{- if empty .Values.managed.objects.endpoint -}}
  {{- fail "managed.objects.endpoint is required for managed storage" -}}
  {{- end -}}
  {{- if empty .Values.managed.objects.bucket -}}
  {{- fail "managed.objects.bucket is required for managed storage" -}}
  {{- end -}}
  {{- if or (empty .Values.secrets.encryptionKey.name) (empty .Values.secrets.encryptionKey.key) -}}
  {{- fail "secrets.encryptionKey.name and key are required for managed storage" -}}
  {{- end -}}
  {{- if or (empty .Values.managed.objects.accessKeySecretRef.name) (empty .Values.managed.objects.accessKeySecretRef.key) -}}
  {{- fail "managed.objects.accessKeySecretRef.name and key are required for managed storage" -}}
  {{- end -}}
  {{- if or (empty .Values.managed.objects.secretKeySecretRef.name) (empty .Values.managed.objects.secretKeySecretRef.key) -}}
  {{- fail "managed.objects.secretKeySecretRef.name and key are required for managed storage" -}}
  {{- end -}}
  {{- if ne (empty .Values.managed.objects.sessionTokenSecretRef.name) (empty .Values.managed.objects.sessionTokenSecretRef.key) -}}
  {{- fail "managed.objects.sessionTokenSecretRef.name and key must be set together" -}}
  {{- end -}}
{{- else -}}
{{- fail "storage.profile must be embedded or managed" -}}
{{- end -}}
{{- end }}
