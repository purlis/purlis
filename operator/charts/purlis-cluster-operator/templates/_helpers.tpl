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

{{- /*
The fields of the pinned upstream's pod spec, and of a container's, but those the operator's own
template must set or a renewal may change (operator/tests/the_install_manifests.rs holds these to
the vendored definitions, so a new one fails there when the pin moves).
*/ -}}
{{- define "purlis-cluster-operator.pod-fields" -}}
activeDeadlineSeconds affinity dnsConfig dnsPolicy ephemeralContainers evictionResponders hostAliases hostIPC hostNetwork hostPID hostUsers hostname hostnameOverride imagePullSecrets nodeName nodeSelector os overhead preemptionPolicy priority priorityClassName readinessGates resourceClaims resources restartPolicy runtimeClassName schedulerName schedulingGates schedulingGroup setHostnameAsFQDN shareProcessNamespace subdomain terminationGracePeriodSeconds tolerations topologySpreadConstraints
{{- end }}

{{- define "purlis-cluster-operator.container-fields" -}}
command envFrom imagePullPolicy lifecycle livenessProbe ports readinessProbe resizePolicy restartPolicy restartPolicyRules startupProbe stdin stdinOnce tty volumeDevices workingDir
{{- end }}
