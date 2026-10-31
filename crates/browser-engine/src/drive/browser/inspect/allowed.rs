// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What DevTools' front end may ask of the inspected page: an allow-list** (#2028).
//!
//! Every message the front end sends is checked here before it reaches the page, and anything
//! not listed is refused: every method is listed by name, so any other method, of any domain,
//! a method a later engine adds included, is refused until it is listed here; and so is a call
//! whose origin is not the inspected page's. A refusal is DevTools' own error to show, never a
//! crash.

use std::collections::HashSet;

use serde_json::Value;

use crate::WebAddress;

/// What a call is checked against: the origins of the inspected page's frames, and the
/// execution contexts of purlis's own worlds in it, which DevTools never reach.
pub(crate) struct Reach<'a> {
    pub origins: &'a HashSet<String>,
    pub ours: &'a HashSet<i64>,
    pub ours_unique: &'a HashSet<String>,
    /// The scripts of purlis's own worlds in the page, by id.
    pub our_scripts: &'a HashSet<String>,
}

/// The methods allowed of the domains listed by method.
const METHODS: &[(&str, &[&str])] = &[
    (
        "Accessibility",
        &[
            "enable",
            "disable",
            "getPartialAXTree",
            "getFullAXTree",
            "getRootAXNode",
            "getAXNodeAndAncestors",
            "getChildAXNodes",
            "queryAXTree",
        ],
    ),
    (
        "Animation",
        &[
            "enable",
            "disable",
            "getCurrentTime",
            "getPlaybackRate",
            "releaseAnimations",
            "resolveAnimation",
            "seekAnimations",
            "setPaused",
            "setPlaybackRate",
            "setTiming",
        ],
    ),
    (
        "Audits",
        &[
            "enable",
            "disable",
            "getEncodedResponse",
            "checkContrast",
            "checkFormsIssues",
        ],
    ),
    (
        "CSS",
        &[
            "enable",
            "disable",
            "addRule",
            "collectClassNames",
            "createStyleSheet",
            "forcePseudoState",
            "forceStartingStyle",
            "getBackgroundColors",
            "getComputedStyleForNode",
            "resolveValues",
            "getLonghandProperties",
            "getInlineStylesForNode",
            "getAnimatedStylesForNode",
            "getMatchedStylesForNode",
            "getEnvironmentVariables",
            "getMediaQueries",
            "getPlatformFontsForNode",
            "getStyleSheetText",
            "getLayersForNode",
            "getLocationForSelector",
            "trackComputedStyleUpdatesForNode",
            "trackComputedStyleUpdates",
            "takeComputedStyleUpdates",
            "setEffectivePropertyValueForNode",
            "setPropertyRulePropertyName",
            "setKeyframeKey",
            "setMediaText",
            "setContainerQueryText",
            "setSupportsText",
            "setScopeText",
            "setRuleSelector",
            "setStyleSheetText",
            "setStyleTexts",
            "startRuleUsageTracking",
            "stopRuleUsageTracking",
            "takeCoverageDelta",
            "setLocalFontsEnabled",
        ],
    ),
    (
        "DOMDebugger",
        &[
            "getEventListeners",
            "removeDOMBreakpoint",
            "removeEventListenerBreakpoint",
            "removeInstrumentationBreakpoint",
            "removeXHRBreakpoint",
            "setBreakOnCSPViolation",
            "setDOMBreakpoint",
            "setEventListenerBreakpoint",
            "setInstrumentationBreakpoint",
            "setXHRBreakpoint",
        ],
    ),
    (
        "DOMSnapshot",
        &["enable", "disable", "getSnapshot", "captureSnapshot"],
    ),
    (
        "DeviceOrientation",
        &[
            "clearDeviceOrientationOverride",
            "setDeviceOrientationOverride",
        ],
    ),
    (
        "Emulation",
        &[
            "canEmulate",
            "clearDeviceMetricsOverride",
            "clearGeolocationOverride",
            "resetPageScaleFactor",
            "setFocusEmulationEnabled",
            "setAutoDarkModeOverride",
            "setCPUThrottlingRate",
            "setDefaultBackgroundColorOverride",
            "setSafeAreaInsetsOverride",
            "setDeviceMetricsOverride",
            "setDevicePostureOverride",
            "clearDevicePostureOverride",
            "setDisplayFeaturesOverride",
            "clearDisplayFeaturesOverride",
            "setScrollbarsHidden",
            "setDocumentCookieDisabled",
            "setEmitTouchEventsForMouse",
            "setEmulatedMedia",
            "setEmulatedVisionDeficiency",
            "setGeolocationOverride",
            "getOverriddenSensorInformation",
            "setSensorOverrideEnabled",
            "setSensorOverrideReadings",
            "setPressureSourceOverrideEnabled",
            "setPressureStateOverride",
            "setPressureDataOverride",
            "setIdleOverride",
            "clearIdleOverride",
            "setPageScaleFactor",
            "setScriptExecutionDisabled",
            "setTouchEmulationEnabled",
            "setLocaleOverride",
            "setTimezoneOverride",
            "setDisabledImageTypes",
            "setHardwareConcurrencyOverride",
            "setUserAgentOverride",
            "setAutomationOverride",
            "setSmallViewportHeightDifferenceOverride",
        ],
    ),
    (
        "EventBreakpoints",
        &[
            "setInstrumentationBreakpoint",
            "removeInstrumentationBreakpoint",
            "disable",
        ],
    ),
    (
        "HeapProfiler",
        &[
            "enable",
            "disable",
            "collectGarbage",
            "getHeapObjectId",
            "getSamplingProfile",
            "startSampling",
            "startTrackingHeapObjects",
            "stopSampling",
            "stopTrackingHeapObjects",
            "takeHeapSnapshot",
        ],
    ),
    ("Inspector", &["enable", "disable"]),
    (
        "LayerTree",
        &[
            "enable",
            "disable",
            "compositingReasons",
            "loadSnapshot",
            "makeSnapshot",
            "profileSnapshot",
            "releaseSnapshot",
            "replaySnapshot",
            "snapshotCommandLog",
        ],
    ),
    (
        "Log",
        &[
            "enable",
            "disable",
            "clear",
            "startViolationsReport",
            "stopViolationsReport",
        ],
    ),
    ("Media", &["enable", "disable"]),
    (
        "Overlay",
        &[
            "enable",
            "disable",
            "getGridHighlightObjectsForTest",
            "getHighlightObjectForTest",
            "getSourceOrderHighlightObjectForTest",
            "hideHighlight",
            "highlightFrame",
            "highlightNode",
            "highlightQuad",
            "highlightRect",
            "highlightSourceOrder",
            "setInspectMode",
            "setShowAdHighlights",
            "setPausedInDebuggerMessage",
            "setShowDebugBorders",
            "setShowFPSCounter",
            "setShowGridOverlays",
            "setShowFlexOverlays",
            "setShowScrollSnapOverlays",
            "setShowContainerQueryOverlays",
            "setShowPaintRects",
            "setShowLayoutShiftRegions",
            "setShowScrollBottleneckRects",
            "setShowHitTestBorders",
            "setShowWebVitals",
            "setShowViewportSizeOnResize",
            "setShowHinge",
            "setShowIsolatedElements",
            "setShowWindowControlsOverlay",
        ],
    ),
    (
        "Performance",
        &["enable", "disable", "getMetrics", "setTimeDomain"],
    ),
    ("PerformanceTimeline", &["enable"]),
    (
        "Profiler",
        &[
            "enable",
            "disable",
            "getBestEffortCoverage",
            "setSamplingInterval",
            "start",
            "startPreciseCoverage",
            "stop",
            "stopPreciseCoverage",
            "takePreciseCoverage",
        ],
    ),
    ("Schema", &["getDomains"]),
    ("WebAudio", &["enable", "disable", "getRealtimeData"]),
    (
        "Runtime",
        &[
            "enable",
            "disable",
            "evaluate",
            "callFunctionOn",
            "getProperties",
            "releaseObject",
            "releaseObjectGroup",
            "compileScript",
            "runScript",
            "awaitPromise",
            "queryObjects",
            "globalLexicalScopeNames",
            "getHeapUsage",
            "getIsolateId",
            "getExceptionDetails",
            "runIfWaitingForDebugger",
            "setAsyncCallStackDepth",
            "setCustomObjectFormatterEnabled",
            "setMaxCallStackSizeToCapture",
            "discardConsoleEntries",
            "terminateExecution",
        ],
    ),
    (
        "Debugger",
        &[
            "enable",
            "disable",
            "continueToLocation",
            "evaluateOnCallFrame",
            "getPossibleBreakpoints",
            "getScriptSource",
            "getStackTrace",
            "disassembleWasmModule",
            "nextWasmDisassemblyChunk",
            "pause",
            "removeBreakpoint",
            "restartFrame",
            "resume",
            "searchInContent",
            "setAsyncCallStackDepth",
            "setBlackboxExecutionContexts",
            "setBlackboxPatterns",
            "setBlackboxedRanges",
            "setBreakpoint",
            "setBreakpointByUrl",
            "setBreakpointOnFunctionCall",
            "setBreakpointsActive",
            "setInstrumentationBreakpoint",
            "setPauseOnExceptions",
            "setReturnValue",
            "setScriptSource",
            "setSkipAllPauses",
            "setVariableValue",
            "stepInto",
            "stepOut",
            "stepOver",
        ],
    ),
    (
        "DOM",
        &[
            "enable",
            "disable",
            "collectClassNamesFromSubtree",
            "copyTo",
            "describeNode",
            "discardSearchResults",
            "focus",
            "getAttributes",
            "getBoxModel",
            "getContentQuads",
            "getContainerForNode",
            "getDocument",
            "getFlattenedDocument",
            "getFrameOwner",
            "getNodeForLocation",
            "getNodeStackTraces",
            "getNodesForSubtreeByStyle",
            "getOuterHTML",
            "getQueryingDescendantsForContainer",
            "getRelayoutBoundary",
            "getSearchResults",
            "getTopLayerElements",
            "getAnchorElement",
            "getDetachedDomNodes",
            "getElementByRelation",
            "hideHighlight",
            "highlightNode",
            "highlightRect",
            "markUndoableState",
            "moveTo",
            "performSearch",
            "pushNodeByPathToFrontend",
            "pushNodesByBackendIdsToFrontend",
            "querySelector",
            "querySelectorAll",
            "redo",
            "removeAttribute",
            "removeNode",
            "requestChildNodes",
            "requestNode",
            "resolveNode",
            "scrollIntoViewIfNeeded",
            "setAttributeValue",
            "setAttributesAsText",
            "setInspectedNode",
            "setNodeName",
            "setNodeStackTracesEnabled",
            "setNodeValue",
            "setOuterHTML",
            "undo",
            "forceShowPopover",
        ],
    ),
    (
        "Page",
        &[
            "enable",
            "disable",
            "addScriptToEvaluateOnNewDocument",
            "removeScriptToEvaluateOnNewDocument",
            "captureScreenshot",
            "getAppManifest",
            "getAppId",
            "getAdScriptAncestry",
            "getFrameTree",
            "getInstallabilityErrors",
            "getLayoutMetrics",
            "getNavigationHistory",
            "getOriginTrials",
            "getPermissionsPolicyState",
            "getResourceContent",
            "getResourceTree",
            "handleJavaScriptDialog",
            "navigate",
            "reload",
            "resetNavigationHistory",
            "searchInResource",
            "setAdBlockingEnabled",
            "setBypassCSP",
            "setDocumentContent",
            "setLifecycleEventsEnabled",
            "setPrerenderingAllowed",
            "startScreencast",
            "stopScreencast",
            "screencastFrameAck",
            "stopLoading",
            "produceCompilationCache",
            "addCompilationCache",
            "clearCompilationCache",
        ],
    ),
    (
        "Network",
        &[
            "enable",
            "disable",
            "clearAcceptedEncodingsOverride",
            "configureDurableMessages",
            "emulateNetworkConditions",
            "emulateNetworkConditionsByRule",
            "enableReportingApi",
            "getCookies",
            "getRequestPostData",
            "getResponseBody",
            "getSecurityIsolationStatus",
            "loadNetworkResource",
            "overrideNetworkState",
            "replayXHR",
            "searchInResponseBody",
            "setAcceptedEncodings",
            "setAttachDebugStack",
            "setBlockedURLs",
            "setBypassServiceWorker",
            "setCacheDisabled",
            "setExtraHTTPHeaders",
            "setUserAgentOverride",
            "streamResourceContent",
        ],
    ),
    (
        "Storage",
        &[
            "getStorageKey",
            "getStorageKeyForFrame",
            "getUsageAndQuota",
            "clearDataForOrigin",
            "clearDataForStorageKey",
            "getSharedStorageEntries",
            "getSharedStorageMetadata",
            "setStorageBucketTracking",
            "trackCacheStorageForOrigin",
            "trackCacheStorageForStorageKey",
            "trackIndexedDBForOrigin",
            "trackIndexedDBForStorageKey",
            "untrackCacheStorageForOrigin",
            "untrackCacheStorageForStorageKey",
            "untrackIndexedDBForOrigin",
            "untrackIndexedDBForStorageKey",
        ],
    ),
    (
        "DOMStorage",
        &[
            "enable",
            "disable",
            "clear",
            "getDOMStorageItems",
            "removeDOMStorageItem",
            "setDOMStorageItem",
        ],
    ),
    (
        "IndexedDB",
        &[
            "enable",
            "disable",
            "clearObjectStore",
            "deleteDatabase",
            "deleteObjectStoreEntries",
            "getMetadata",
            "requestData",
            "requestDatabase",
            "requestDatabaseNames",
        ],
    ),
    // A cache's contents are named by an id that does not say whose it is: the names alone.
    ("CacheStorage", &["requestCacheNames"]),
    (
        "Target",
        &["setAutoAttach", "detachFromTarget", "getTargetInfo"],
    ),
    (
        "Input",
        &[
            "dispatchKeyEvent",
            "dispatchMouseEvent",
            "dispatchTouchEvent",
            "dispatchDragEvent",
            "emulateTouchFromMouseEvent",
            "imeSetComposition",
            "insertText",
        ],
    ),
    ("IO", &["read", "close", "resolveBlob"]),
    (
        "Memory",
        &[
            "getDOMCounters",
            "getDOMCountersForLeakDetection",
            "prepareForLeakDetection",
            "forciblyPurgeJavaScriptMemory",
        ],
    ),
    ("Security", &["enable", "disable"]),
];

/// The parameters that name an origin, or a storage key that begins with one: each must be one
/// of the inspected page's.
const ORIGIN_PARAMS: &[&str] = &["securityOrigin", "origin", "storageKey", "ownerOrigin"];

/// The parameters that name an execution context: never one of purlis's own worlds.
const CONTEXT_PARAMS: &[&str] = &["contextId", "executionContextId"];

/// Why the front end's call of `method` with `params` is refused, or `None` where it goes to
/// the page.
pub(crate) fn refused(method: &str, params: &Value, reach: &Reach<'_>) -> Option<String> {
    let Some((domain, name)) = method.split_once('.') else {
        return Some(format!("{method} is not a DevTools method"));
    };
    let listed = METHODS
        .iter()
        .any(|(of, names)| *of == domain && names.contains(&name));
    if !listed {
        return Some(format!("{method} is not offered in purlis's DevTools"));
    }
    if let Some(why) = past_the_page(method, params, reach) {
        return Some(format!(
            "{method} is not offered in purlis's DevTools: {why}"
        ));
    }
    None
}

/// Why a listed call reaches past the inspected page, if it does.
fn past_the_page(method: &str, params: &Value, reach: &Reach<'_>) -> Option<&'static str> {
    match method {
        // Only web addresses navigate (spec #2008): the page's own rule.
        "Page.navigate" => {
            let address = params
                .get("url")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if WebAddress::parse(address).is_err() {
                return Some("only http and https addresses open");
            }
        }
        // A resource of the inspected page's own origins alone, a source map's say: never
        // another site's, with its sign-in or without.
        "Network.loadNetworkResource" => {
            let address = params
                .get("url")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if WebAddress::parse(address).is_err() {
                return Some("only http and https addresses load");
            }
            if !origin_root(address).is_some_and(|origin| reach.origins.contains(&origin)) {
                return Some("only the inspected page's own origins are loaded from");
            }
        }
        // The cookies of the inspected page's own origins alone.
        "Network.getCookies" => {
            let urls = params.get("urls").and_then(Value::as_array);
            let theirs = urls.is_some_and(|urls| {
                !urls.is_empty()
                    && urls.iter().all(|url| {
                        url.as_str()
                            .and_then(origin_root)
                            .is_some_and(|origin| reach.origins.contains(&origin))
                    })
            });
            if !theirs {
                return Some("only the inspected page's own cookies are read");
            }
        }
        // Another target's information, by its id.
        "Target.getTargetInfo" if params.get("targetId").is_some() => {
            return Some("only this page's own information is read");
        }
        "Input.dispatchDragEvent" if params.pointer("/data/files").is_some() => {
            return Some("files are not dragged in");
        }
        _ => {}
    }
    if !every_origin_is_theirs(params, reach) {
        return Some("only the inspected page's own origins are reached");
    }
    if names_our_world(params, reach) {
        return Some("purlis's own world is not inspected");
    }
    None
}

/// Whether every origin `params` names, anywhere in them, is one of the inspected page's.
fn every_origin_is_theirs(params: &Value, reach: &Reach<'_>) -> bool {
    match params {
        Value::Object(map) => map.iter().all(|(key, value)| {
            if ORIGIN_PARAMS.contains(&key.as_str())
                && let Some(named) = value.as_str()
            {
                origin_root(named).is_some_and(|origin| reach.origins.contains(&origin))
            } else {
                every_origin_is_theirs(value, reach)
            }
        }),
        Value::Array(values) => values
            .iter()
            .all(|value| every_origin_is_theirs(value, reach)),
        _ => true,
    }
}

/// Whether `params` name one of purlis's own worlds.
fn names_our_world(params: &Value, reach: &Reach<'_>) -> bool {
    match params {
        Value::Object(map) => map.iter().any(|(key, value)| {
            (CONTEXT_PARAMS.contains(&key.as_str())
                && value.as_i64().is_some_and(|id| reach.ours.contains(&id)))
                || (key == "uniqueContextId"
                    && value
                        .as_str()
                        .is_some_and(|unique| reach.ours_unique.contains(unique)))
                || (key == "scriptId"
                    && value
                        .as_str()
                        .is_some_and(|script| reach.our_scripts.contains(script)))
                || names_our_world(value, reach)
        }),
        Value::Array(values) => values.iter().any(|value| names_our_world(value, reach)),
        _ => false,
    }
}

/// The origin an address or storage key begins with (`https://shop.example:8443`), lowercased,
/// for http and https alone.
pub(crate) fn origin_root(address: &str) -> Option<String> {
    let (scheme, rest) = address.split_once("://")?;
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return None;
    }
    let authority = rest.split(['/', '?', '#', '^']).next()?;
    if authority.is_empty() || authority.contains('@') {
        return None;
    }
    Some(format!("{scheme}://{}", authority.to_ascii_lowercase()))
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use serde_json::json;

    use super::{Reach, refused};

    fn origins() -> HashSet<String> {
        ["http://fixture.test".to_owned()].into_iter().collect()
    }

    fn check(method: &str, params: serde_json::Value) -> Option<String> {
        let origins = origins();
        let ours: HashSet<i64> = [7].into_iter().collect();
        let ours_unique: HashSet<String> = ["purlis-world".to_owned()].into_iter().collect();
        let our_scripts: HashSet<String> = ["71".to_owned()].into_iter().collect();
        refused(
            method,
            &params,
            &Reach {
                origins: &origins,
                ours: &ours,
                ours_unique: &ours_unique,
                our_scripts: &our_scripts,
            },
        )
    }

    #[test]
    fn a_method_not_listed_is_refused_whatever_it_is() {
        for method in [
            "Browser.close",
            "SystemInfo.getProcessInfo",
            "Tracing.start",
            "ServiceWorker.unregister",
            "Fetch.enable",
            "Target.createTarget",
            "Target.attachToTarget",
            "Target.setDiscoverTargets",
            "Network.getAllCookies",
            "Network.setCookie",
            "Storage.getCookies",
            "DOM.setFileInputFiles",
            "Page.close",
            "Page.crash",
            "Page.setDownloadBehavior",
            "Runtime.addBinding",
            "Security.setIgnoreCertificateErrors",
            // One no engine has today: refused until it is listed.
            "Network.someMethodALaterEngineAdds",
            "Telepathy.read",
            "notamethod",
        ] {
            assert!(check(method, json!({})).is_some(), "{method} is refused");
        }
    }

    #[test]
    fn what_devtools_need_of_the_page_goes() {
        for method in [
            "DOM.getDocument",
            "CSS.getMatchedStylesForNode",
            "Overlay.highlightNode",
            "Runtime.evaluate",
            "Debugger.enable",
            "Network.enable",
            "Network.getResponseBody",
            "Page.getResourceTree",
            "Target.setAutoAttach",
            "Emulation.setDeviceMetricsOverride",
            "Log.enable",
        ] {
            assert_eq!(check(method, json!({})), None, "{method} goes");
        }
    }

    #[test]
    fn a_call_reaching_another_origin_or_scheme_is_refused() {
        assert!(check("Page.navigate", json!({ "url": "file:///etc/hosts" })).is_some());
        assert!(
            check(
                "Page.navigate",
                json!({ "url": "devtools://devtools/bundled/x.html" })
            )
            .is_some()
        );
        assert_eq!(
            check("Page.navigate", json!({ "url": "https://example.com/" })),
            None
        );
        assert!(check("Network.loadNetworkResource", json!({ "url": "file:///x" })).is_some());
        assert!(check("Network.getCookies", json!({})).is_some());
        assert!(
            check(
                "Network.getCookies",
                json!({ "urls": ["https://bank.test/"] })
            )
            .is_some()
        );
        assert_eq!(
            check(
                "Network.getCookies",
                json!({ "urls": ["http://fixture.test/a"] })
            ),
            None
        );
        assert!(
            check(
                "Storage.clearDataForOrigin",
                json!({ "origin": "https://bank.test", "storageTypes": "all" })
            )
            .is_some()
        );
        assert_eq!(
            check(
                "Storage.clearDataForOrigin",
                json!({ "origin": "http://fixture.test", "storageTypes": "all" })
            ),
            None
        );
        assert!(check("DOMStorage.getDOMStorageItems", json!({ "storageId": { "storageKey": "https://bank.test/", "isLocalStorage": true } })).is_some());
        assert!(
            check(
                "IndexedDB.requestDatabaseNames",
                json!({ "securityOrigin": "https://bank.test" })
            )
            .is_some()
        );
        assert!(check("Target.getTargetInfo", json!({ "targetId": "OTHER" })).is_some());
        assert!(check("Input.dispatchDragEvent", json!({ "type": "drop", "x": 1, "y": 1, "data": { "items": [], "files": ["/etc/hosts"], "dragOperationsMask": 1 } })).is_some());
    }

    #[test]
    fn a_resource_is_loaded_from_the_inspected_pages_own_origins_alone() {
        // Another origin's, with its credentials or without: refused.
        assert!(check("Network.loadNetworkResource", json!({ "url": "https://bank.test/statement", "options": { "includeCredentials": true, "disableCache": true } })).is_some());
        assert!(check("Network.loadNetworkResource", json!({ "url": "https://bank.test/app.js.map", "options": { "includeCredentials": false, "disableCache": true } })).is_some());
        // The page's own: goes.
        assert_eq!(
            check(
                "Network.loadNetworkResource",
                json!({ "url": "http://fixture.test/app.js.map", "options": { "includeCredentials": false, "disableCache": true } })
            ),
            None
        );
    }

    #[test]
    fn a_method_a_later_engine_adds_to_a_domain_devtools_use_is_refused() {
        for method in [
            "CSS.someMethodALaterEngineAdds",
            "Emulation.someMethodALaterEngineAdds",
            "Overlay.someMethodALaterEngineAdds",
            "Accessibility.someMethodALaterEngineAdds",
        ] {
            assert!(check(method, json!({})).is_some(), "{method} is refused");
        }
    }

    #[test]
    fn what_a_cache_id_or_a_lifecycle_state_would_reach_is_refused() {
        for method in [
            "CacheStorage.requestEntries",
            "CacheStorage.requestCachedResponse",
            "CacheStorage.deleteCache",
            "CacheStorage.deleteEntry",
            "Page.setWebLifecycleState",
            "Emulation.setVirtualTimePolicy",
        ] {
            assert!(
                check(method, json!({ "cacheId": "x", "state": "frozen" })).is_some(),
                "{method}"
            );
        }
    }

    #[test]
    fn purlis_own_scripts_and_objects_by_heap_id_are_never_reached() {
        assert!(
            check(
                "Debugger.setBreakpoint",
                json!({ "location": { "scriptId": "71", "lineNumber": 0 } })
            )
            .is_some()
        );
        assert!(check("Debugger.getScriptSource", json!({ "scriptId": "71" })).is_some());
        assert_eq!(
            check("Debugger.getScriptSource", json!({ "scriptId": "72" })),
            None
        );
        assert!(
            check(
                "HeapProfiler.getObjectByHeapObjectId",
                json!({ "objectId": "1" })
            )
            .is_some()
        );
        assert!(
            check(
                "HeapProfiler.addInspectedHeapObject",
                json!({ "heapObjectId": "1" })
            )
            .is_some()
        );
    }

    #[test]
    fn purlis_own_world_is_never_reached() {
        assert!(
            check(
                "Runtime.evaluate",
                json!({ "expression": "1", "contextId": 7 })
            )
            .is_some()
        );
        assert!(
            check(
                "Runtime.evaluate",
                json!({ "expression": "1", "uniqueContextId": "purlis-world" })
            )
            .is_some()
        );
        assert!(check("Runtime.compileScript", json!({ "expression": "1", "sourceURL": "", "persistScript": false, "executionContextId": 7 })).is_some());
        assert_eq!(
            check(
                "Runtime.evaluate",
                json!({ "expression": "1", "contextId": 3 })
            ),
            None
        );
    }
}
