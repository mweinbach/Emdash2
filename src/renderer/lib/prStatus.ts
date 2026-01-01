export type PrInfo = {
  number?: number;
  title?: string;
  url?: string;
  state?: string | null;
  isDraft?: boolean;
};

export type PrChecksSummary = {
  total: number;
  passed: number;
  failed: number;
  pending: number;
};

export type PrComment = {
  id?: string;
  type?: 'comment' | 'review';
  author?: { login?: string; name?: string } | null;
  body?: string | null;
  createdAt?: string | null;
  url?: string | null;
  state?: string | null;
};

export type PrCommit = {
  oid?: string | null;
  shortOid?: string | null;
  message?: string | null;
  author?: string | null;
  date?: string | null;
};

export type PrFile = {
  path: string;
  additions?: number | null;
  deletions?: number | null;
  changeType?: string | null;
};

export type PrStatus = PrInfo & {
  mergeStateStatus?: string;
  commentsCount?: number;
  reviewCount?: number;
  checksSummary?: PrChecksSummary;
  headRefName?: string;
  baseRefName?: string;
  additions?: number;
  deletions?: number;
  changedFiles?: number;
};

export const isActivePr = (pr?: PrInfo | null): pr is PrInfo => {
  if (!pr) return false;
  const state = typeof pr?.state === 'string' ? pr.state.toLowerCase() : '';
  if (state === 'merged' || state === 'closed') return false;
  return true;
};
