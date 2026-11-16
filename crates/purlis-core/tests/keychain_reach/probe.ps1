# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# R-772's probe on Windows (T1a-72): what a chat's command, hook or MCP server reaches of the
# Credential Manager. The same modes and answers as probe.sh:
#
#   probe.ps1 [-As command|hook|mcp] reach
#   probe.ps1 [-As command|hook|mcp] stub <file>...
#   probe.ps1 [-As command|hook|mcp] fetch <target>
#
# purlis has no sandbox backend on Windows, so every caller runs as the person. `reach` calls
# CredRead for a target nothing holds: an answer of ERROR_NOT_FOUND (1168) is the API answering.
# A fetch never prints the value it read.
param(
    [string]$As = "command",
    [Parameter(Position = 0)][string]$Mode,
    [Parameter(Position = 1, ValueFromRemainingArguments = $true)][string[]]$Rest
)

if ($As -eq "mcp") { [void][Console]::In.ReadLine() }
"caller=$As"

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class PurlisProbeCred {
    [DllImport("advapi32.dll", EntryPoint = "CredReadW", CharSet = CharSet.Unicode, SetLastError = true)]
    static extern bool CredRead(string target, int type, int flags, out IntPtr credential);
    [DllImport("advapi32.dll")]
    static extern void CredFree(IntPtr credential);
    public static int Read(string target) {
        IntPtr credential;
        if (CredRead(target, 1, 0, out credential)) { CredFree(credential); return 0; }
        return Marshal.GetLastWin32Error();
    }
}
"@

switch ($Mode) {
    "reach" {
        $code = [PurlisProbeCred]::Read("purlis/@reach-probe/" + [guid]::NewGuid())
        if ($code -eq 1168 -or $code -eq 0) { "service=reachable" } else { "service=denied"; "code=$code" }
    }
    "stub" {
        $read = "denied"; $write = "denied"
        foreach ($file in $Rest) {
            try { [void][IO.File]::ReadAllBytes($file); $read = "reachable" } catch {}
            try { $s = [IO.File]::Open($file, "Append", "Write"); $s.Close(); $write = "reachable" } catch {}
        }
        "stub-read=$read"; "stub-write=$write"
    }
    "fetch" {
        $code = [PurlisProbeCred]::Read($Rest[0])
        switch ($code) {
            0 { "item=readable"; "code=0" }
            1168 { "item=denied"; "code=1168 ERROR_NOT_FOUND" }
            default { "item=denied"; "code=$code" }
        }
    }
    default { [Console]::Error.WriteLine("usage: probe.ps1 [-As command|hook|mcp] reach | stub <file>... | fetch <target>"); exit 2 }
}
