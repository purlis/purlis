# SARIF fixtures

What `crates/purlis-core/tests/it/check_results_on_a_comparison.rs` reads (#5407, ADR 0114).

- **`codeql-2.27.4-actions.sarif`**: CodeQL 2.27.4's real output, this repository's own analysis
  2104455872 from `GET /repos/purlis/purlis/code-scanning/analyses/<id>` with
  `Accept: application/sarif+json`. Cut to its first three results, without their `codeFlows` and
  `relatedLocations`, and without the rule lists and all but the first extension.
- **`scorecard-5.6.0.sarif`**: OpenSSF Scorecard 5.6.0's real output, analysis 2076983530, fetched
  the same way, without its rules' `help` and `fullDescription`. Two of its results name the path
  `no file associated with this alert`, as Scorecard really writes them.
- **`malformed/`**: written by hand, one defect each.

Never edit a captured result to suit a test: capture another, and say here where it came from.
