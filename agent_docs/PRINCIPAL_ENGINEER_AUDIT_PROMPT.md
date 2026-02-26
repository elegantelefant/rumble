# Principal Engineer Code Audit Prompt

## Overview

Perform a comprehensive code audit of a Vue 3/Nuxt 3 application, systematically analyzing every user interaction across all pages to identify bugs, architectural issues, and overcomplicated implementations.

## Instructions

### Phase 1: Systematic Page & Interaction Analysis

1. **Find All Pages**: Discover all pages in the application (typically in `/pages` directory)
2. **Create Master Checklist**: Document every page with status tracking
3. **Analyze Interactions**: For each page, identify all user interactions:
   - Button clicks
   - Form inputs and submissions
   - Keyboard shortcuts and hotkeys
   - Authentication flows
   - Navigation links
   - Drag-and-drop operations
   - Any other user-triggered behavior

4. **Track Progress**: Use checkmarks (✓/✗) to track completion:
   - Each page has a checkbox
   - Each interaction has a checkbox
   - Check off as you analyze

5. **Document Findings**: For each interaction, note:
   - Expected behavior
   - Actual code path (which functions/components are called)
   - Whether code will run (✓ WORKS, ✗ FAILS, ⚠️ ISSUE)
   - Line numbers and file references
   - Any issues discovered

### Phase 2: Code Quality Assessment

6. **Identify Critical Bugs**: Flag issues that break functionality:
   - Missing imports
   - Undefined functions
   - Type errors
   - Logic errors
   - Race conditions
   - Memory leaks

7. **Assess Code Complexity**: For each page/component, note:
   - Overcomplicated implementations
   - Repeated code patterns (DRY violations)
   - Mixed concerns (should be separated)
   - Manual implementations of standard patterns
   - Scattered state management
   - Dead code (commented code, unused functions)

8. **Categorize Severity**:
   - 🔴 CRITICAL: Blocks app functionality, prevents startup
   - 🟡 MEDIUM: Causes bugs, memory leaks, poor UX
   - 🟢 LOW: Technical debt, code quality issues

### Phase 3: Implementation Planning

9. **Create Fix Roadmap**:
   - **Phase 1 (Critical Fixes)**: Must fix first - app won't run
   - **Phase 2 (Quick Wins)**: High-impact, low-effort improvements
   - **Phase 3 (Medium Effort)**: Refactoring improvements

10. **For Each Fix, Document**:
    - File location and line numbers
    - Current problem code (with snippet)
    - Proposed solution (with snippet)
    - Step-by-step implementation instructions
    - Testing checklist
    - Time estimate
    - Risk assessment

## Output Format

Save findings to a single append-only document in `/agent_docs/`:

```
# Application Audit Report

## MASTER PAGE CHECKLIST
- [x] Page Name (Category)
- [ ] Page Name (Category)

## DETAILED INTERACTION AUDIT

### PAGE: /path/to/page.vue

#### Interactions Checklist:
- [x] Interaction Name
  - User action → Component → Function → Result
  - Status: ✓ WORKS / ✗ FAILS / ⚠️ ISSUE
  - Issue: (if any)

## CODE QUALITY FINDINGS

### Critical Issues
- Issue #1: [Description with file:line reference]
- Issue #2: [Description with file:line reference]

### Complexity Analysis
- Component A: [Pattern overcomplicated in which way]
- Component B: [Pattern overcomplicated in which way]

## IMPLEMENTATION PLAN

### Phase 1: Critical Fixes (Time estimate)
#### Fix #1: [Title]
- File: /path/to/file.vue
- Problem: [code snippet]
- Solution: [code snippet]
- Steps: [numbered list]
- Time: X minutes
- Risk: 🟢 None / 🟡 Low / 🔴 High

### Phase 2: Quick Wins (Time estimate)
[Similar structure]

### Phase 3: Medium Effort (Time estimate)
[Similar structure]
```

## Key Standards

- **Line References**: Always note `file.ts:line_number` for code locations
- **Code Snippets**: Include problem and solution code for clarity
- **Checkmarks**: Use consistent status indicators (✓/✗/⚠️)
- **Hierarchy**: Organize by page → interaction → component → function
- **Completeness**: Don't skip pages; track what you've analyzed
- **Actionability**: Every fix should have clear implementation steps

## Success Criteria

- [ ] All pages discovered and listed
- [ ] Every user interaction traced to code
- [ ] Critical bugs identified with exact locations
- [ ] Code complexity patterns documented
- [ ] Implementation plan includes all fixes with time estimates
- [ ] Each fix has clear steps, testing checklist, and risk assessment
- [ ] Total time estimate provided for all phases

## Notes

- Start from the home/entry point and follow user flows
- Don't assume code works - verify by reading implementation
- If unsure about a code path, mark as "NOT ANALYZED" and note why
- Look for common overcomplexity patterns:
  - Reinvented utilities (debounce, timers, validation)
  - Scattered state management
  - Repeated logic across files
  - Dead/commented code
  - Guard clause chains
  - Hardcoded configuration
  - Mixed concerns in components
- Use git history if needed to understand intent
- Document observations that could help future debugging
