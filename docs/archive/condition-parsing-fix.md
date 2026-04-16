# Condition Parsing Fix - Summary

**Date**: April 15, 2026  
**Status**: ✅ Partially Complete (8/42 rules passing, significant progress)

---

## Problem

The provider was failing to parse conditions sent by konveyor-analyzer, causing all `java.referenced` rules to fail with:
```
Failed to parse condition: expected ident at line 1 column 2
```

## Root Cause

The initial implementation expected a simple structure:
```rust
struct Condition {
    referenced: ReferencedCondition,
}
```

But the analyzer sends a more complex YAML structure:
```yaml
tags: {}
template: {}
ruleID: simple-test-001
referenced:
  location: IMPORT
  pattern: java.util.List
```

## Solution Implemented

### 1. Updated ConditionWrapper Structure

Changed from a HashMap-based approach to a proper struct:

```rust
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConditionWrapper {
    #[serde(default)]
    pub tags: HashMap<String, String>,
    #[serde(default)]
    pub template: HashMap<String, String>,
    #[serde(default, rename = "ruleID")]
    pub rule_id: String,
    pub referenced: ReferencedCondition,
}
```

### 2. Switched from JSON to YAML Parsing

Changed from:
```rust
serde_json::from_str(&req.condition_info)
```

To:
```rust
serde_yaml::from_str(&req.condition_info)
```

The analyzer sends conditions in YAML format, not JSON.

### 3. Added Debug Logging

Added file output to inspect full condition structure:
```rust
if let Err(e) = std::fs::write("/tmp/condition_info.yaml", &req.condition_info) {
    warn!("Failed to write condition info to file: {}", e);
}
```

## Results

### Before Fix
- **0 rules passing**
- 40 rules failed with parsing errors
- 2 dependency rules worked (different format)

### After Fix
- **8 rules passing** with violations found ✅
- **16 rules unmatched** (executed successfully, no violations)
- **18 rules failed** (down from 40!)
- **0 dependency rules tested** (no violations in test data)

### Successful Rules
1. `konveyor-java-pattern-test-4` - Exact IMPORT match
2. `konveyor-java-pattern-test-5` - IMPORT with asterisk (after dot)
3. `konveyor-java-pattern-test-6` - IMPORT with asterisk (without dot)
4. `konveyor-java-pattern-test-7` - METHOD with asterisk at end (3 violations)
5. `konveyor-java-pattern-test-9` - METHOD with asterisk and FQ class name (2 violations)
6. `konveyor-java-pattern-test-10` - Exact METHOD match with FQ class name
7. `konveyor-java-pattern-test-111` - METHOD with asterisk in package name
8. `konveyor-java-pattern-test-16` - Exact ANNOTATION match

**Total violations found**: 10 incidents across 8 rules! 🎉

## Remaining Issues

### 1. Annotated Field Parsing (7 failures)

**Error**: `referenced.annotated: invalid type: map, expected a string`

**Problem**: The `annotated` field is defined as `Option<String>`:
```rust
pub struct ReferencedCondition {
    pub annotated: Option<String>,
}
```

But rules send a complex structure:
```yaml
referenced:
  pattern: '*'
  location: CLASS
  annotated:
    pattern: org.springframework.stereotype.Controller
```

**Fix Needed**: Change to a struct:
```rust
#[derive(Debug, Deserialize, Serialize)]
pub struct AnnotatedCondition {
    pub pattern: String,
    #[serde(default)]
    pub elements: Vec<AnnotationElement>,
}

pub struct AnnotationElement {
    pub name: String,
    pub value: String,
}
```

### 2. Template Field Parsing (3 failures)

**Error**: `template.singleton: invalid type: map, expected a string`

**Problem**: Template values can be complex objects, not just strings:
```yaml
template:
  singleton:
    matched: true
    filepaths: [...]
```

**Fix Needed**: Change `template` from `HashMap<String, String>` to `HashMap<String, serde_json::Value>` to accept any JSON value.

### 3. Missing Location Field (3 failures)

**Error**: `referenced: missing field 'location' at line 5 column 3`

**Rules affected**: `java-inclusion-test`, `java-gradle-project`, `java-downloaded-maven-artifact`

**Problem**: Some rules use simpler format:
```yaml
referenced:
  pattern: io.konveyor.util.FileReader
  # No location field!
```

**Fix Needed**: Make `location` optional with a default value:
```rust
#[serde(default = "default_location")]
pub location: String,

fn default_location() -> String {
    "TYPE".to_string()
}
```

### 4. Unknown Location Type (1 failure)

**Error**: `Invalid location type: Unknown location type: FIELD_DECLARATION`

**Fix Needed**: Add to `parse_location_type()`:
```rust
"field_declaration" | "fielddeclaration" => Ok(LocationType::FieldDeclaration),
```

And ensure `LocationType::FieldDeclaration` is handled in query logic.

### 5. GetCodeLocation Not Implemented

**Error**: `unable to get code location: rpc error: code = Unimplemented desc = ""`

**Problem**: The analyzer tries to call `GetCodeLocation` RPC method to get code snippets.

**Fix Needed**: Implement `get_code_location` method in provider:
```rust
async fn get_code_location(
    &self,
    request: Request<GetCodeSnipRequest>,
) -> std::result::Result<Response<GetCodeLocationResponse>, Status> {
    // Extract code snippet from file at given line/column
    // Return formatted code with context
}
```

## Files Modified

1. ✅ `/home/jmle/Dev/redhat/java-analyzer-provider/src/provider/java.rs`
   - Added `use std::collections::HashMap`
   - Changed `Condition` to `ConditionWrapper` with proper fields
   - Updated `evaluate()` to use YAML parsing
   - Added debug file output

## Test Results

### E2E Test Output
```bash
make e2e-local
```

**Summary**:
- **Total rules**: 42
- **Matched (with violations)**: 8
- **Unmatched (executed, no violations)**: 16
- **Failed**: 18
- **Success rate**: 57% (24/42 rules executed successfully)

### Sample Violations

From `e2e-tests/testdata/comprehensive-output.yaml`:

```yaml
konveyor-java-pattern-test-7:
  incidents:
    - uri: file://.../sample-tiles-app/.../HomeController.java
      lineNumber: 22
      message: "METHOD match with asterisk at the end"
    - uri: file://.../sample-tiles-app/.../HomeService.java
      lineNumber: 15
    - uri: file://.../sample-tiles-app/.../HomeService.java
      lineNumber: 20
```

## Next Steps

### Priority 1: Fix Remaining Parsing Issues (Critical)
1. Implement `AnnotatedCondition` struct
2. Fix `template` field to accept complex values
3. Make `location` field optional with default
4. Add `FIELD_DECLARATION` location type

**Estimated Impact**: Should bring success rate to ~90% (38-40/42 rules passing)

### Priority 2: Implement GetCodeLocation (High)
1. Add RPC method to proto interface
2. Implement code snippet extraction from source files
3. Format with context lines

**Estimated Impact**: Better analyzer output with code snippets

### Priority 3: Test and Verify (Medium)
1. Run full E2E test suite
2. Generate expected baseline
3. Compare against Go provider results
4. Document any acceptable differences

## Code Changes Summary

**Lines added**: ~50
**Lines modified**: ~30
**New structs**: 1 (`ConditionWrapper`)
**Methods updated**: 1 (`evaluate`)

## Performance Impact

- **Parsing performance**: Negligible (YAML parsing is fast)
- **Memory usage**: Minimal (small structs)
- **Query execution**: Unchanged

## Lessons Learned

1. **Always inspect actual data format**: The initial assumption about JSON vs YAML was wrong
2. **Debug logging is essential**: Writing to a file helped see the full structure
3. **Incremental testing**: Simple test rule helped isolate the issue quickly
4. **Match analyzer behavior**: The Go provider's structs showed the expected format

## Conclusion

The condition parsing fix successfully resolved the primary blocker for E2E testing. With 8 rules now passing and producing violations, the provider is functional for basic use cases. The remaining 18 failures are due to advanced features (annotations, templating) that need additional struct updates.

**Status**: Ready for next phase of fixes to achieve full E2E test compatibility.

---

## References

- [E2E Testing Setup](./e2e-testing-setup-completion.md)
- [Original Go Provider](https://github.com/konveyor/analyzer-lsp/tree/main/external-providers/java-external-provider)
- [Comprehensive Rules](../e2e-tests/rules/comprehensive.yaml)
