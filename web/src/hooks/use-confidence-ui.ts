import type { Confidence } from '@/lib/types';

export interface ConfidenceUI {
  /** Tailwind border color class */
  borderColor: string;
  /** Tailwind background color class */
  bgColor: string;
  /** Badge text */
  badgeText: string;
  /** Badge classes */
  badgeClass: string;
  /** Show quick-accept button (high confidence only) */
  showQuickAccept: boolean;
  /** Expand output details by default (low confidence) */
  expandDetailsByDefault: boolean;
  /** Force full review before accept (low confidence) */
  forceFullReview: boolean;
  /** Tier: 'high' | 'medium' | 'low' */
  tier: 'high' | 'medium' | 'low';
}

/**
 * Returns adaptive UI config based on confidence score thresholds.
 *
 * High (≥0.8): Green, "AI Confident", quick accept enabled, collapsed details
 * Medium (0.5-0.8): Default, "Review", normal flow
 * Low (<0.5): Amber/red, "Review Carefully", expanded details, no quick accept
 */
export function useConfidenceUI(confidence: Confidence | undefined): ConfidenceUI {
  if (!confidence) {
    return {
      borderColor: 'border-slate-700',
      bgColor: '',
      badgeText: '',
      badgeClass: '',
      showQuickAccept: false,
      expandDetailsByDefault: false,
      forceFullReview: false,
      tier: 'medium',
    };
  }

  const { score } = confidence;

  if (score >= 0.8) {
    return {
      borderColor: 'border-green-500/40',
      bgColor: 'bg-green-500/5',
      badgeText: 'AI Confident',
      badgeClass: 'bg-green-500/20 text-green-400',
      showQuickAccept: true,
      expandDetailsByDefault: false,
      forceFullReview: false,
      tier: 'high',
    };
  }

  if (score >= 0.5) {
    return {
      borderColor: 'border-slate-600',
      bgColor: '',
      badgeText: 'Review',
      badgeClass: 'bg-slate-500/20 text-slate-400',
      showQuickAccept: false,
      expandDetailsByDefault: false,
      forceFullReview: false,
      tier: 'medium',
    };
  }

  return {
    borderColor: 'border-amber-500/50',
    bgColor: 'bg-amber-500/5',
    badgeText: 'Review Carefully',
    badgeClass: 'bg-amber-500/20 text-amber-400',
    showQuickAccept: false,
    expandDetailsByDefault: true,
    forceFullReview: true,
    tier: 'low',
  };
}
