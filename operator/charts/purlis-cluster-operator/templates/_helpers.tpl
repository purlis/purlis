{{- /*
Copyright 2026 Purlis
SPDX-License-Identifier: Apache-2.0
*/ -}}

{{- /*
The labels on everything the chart makes. Nothing in them names Helm, so the plain manifests
rendered from this chart (operator/deploy/) make exactly the objects a Helm install makes.
*/ -}}
{{- define "purlis-cluster-operator.labels" -}}
app.kubernetes.io/name: purlis-cluster-operator
app.kubernetes.io/part-of: purlis
{{- end }}

{{- /*
The operator's image, by digest only: a tag can move.
*/ -}}
{{- define "purlis-cluster-operator.image" -}}
{{- $digest := required "image.digest is required: the operator's image is named by digest (sha256:...), as .github/workflows/operator-image.yml records it" .Values.image.digest -}}
{{- if not (regexMatch "^sha256:[0-9a-f]{64}$" $digest) -}}
{{- fail "image.digest is sha256: and 64 hex digits" -}}
{{- end -}}
{{- printf "%s@%s" .Values.image.repository $digest -}}
{{- end }}
