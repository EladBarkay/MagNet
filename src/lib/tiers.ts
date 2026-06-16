/** Display label + badge styling for an entitlement tier, shared by the toolbar
 *  badge and the settings dialog so the two never drift. */

export const tierLabel = (tier: string): string =>
  ({ free: "Free", pro: "Pro", studio: "Studio" } as Record<string, string>)[tier] ?? tier;

export const tierColor = (tier: string): string =>
  ({
    free: "bg-neutral-700 text-neutral-300",
    pro: "bg-green-700/80 text-white",
    studio: "bg-purple-700/80 text-white",
  } as Record<string, string>)[tier] ?? "bg-neutral-700 text-neutral-300";
