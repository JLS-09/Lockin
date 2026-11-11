// Short labels the week view shows for a class block.

import type { ClassBlock, FilterRule } from "$lib/api";

// UGent's usual short names for the kinds of class, as in the mockups ("HOC · 08:30–10:30").
const kindAbbreviations: Record<string, string> = {
  hoorcollege: "HOC",
  werkcollege: "WPO",
};

/** "HOC", "WPO", or the kind as TimeEdit names it when there's no usual short name. */
export function kindLabel(kind: string | null): string | null {
  if (!kind) return null;
  return kindAbbreviations[kind.toLowerCase()] ?? kind;
}

/**
 * The rule that ignores this block, for "Ignored by rule “zelfstandig werk”".
 * The same check as `filter::is_ignored` in lockin-core: case-insensitive "contains".
 * None when the block was ignored by hand, or isn't ignored.
 */
export function ignoringRule(block: ClassBlock, rules: FilterRule[]): FilterRule | undefined {
  if (block.ignoredOverride !== null || !block.ignoredByRule) return undefined;
  return rules.find((rule) => {
    const value = rule.field === "title" ? block.title : rule.field === "location" ? block.location : null;
    return value?.toLowerCase().includes(rule.pattern.toLowerCase()) ?? false;
  });
}
