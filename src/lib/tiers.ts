/** Display label + badge styling for an entitlement tier, shared by the toolbar
 *  badge and the settings dialog so the two never drift. */

export const TIER_LABELS: Record<string, string> = {
  free: "Free",
  pro: "Pro",
  studio: "Studio",
};

export const TIER_COLORS: Record<string, string> = {
  free: "bg-neutral-700 text-neutral-300",
  pro: "bg-green-700/80 text-white",
  studio: "bg-purple-700/80 text-white",
};

export const tierLabel = (tier: string): string => TIER_LABELS[tier] ?? tier;
export const tierColor = (tier: string): string => TIER_COLORS[tier] ?? TIER_COLORS.free;
