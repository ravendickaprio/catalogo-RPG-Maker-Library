/** Monotonic token generator used to discard stale async responses. */
export class RequestSequencer {
  private seq = 0;

  /** Starts a new request; invalidates all previous tokens. */
  next(): number {
    return ++this.seq;
  }

  current(): number {
    return this.seq;
  }

  isLatest(token: number): boolean {
    return token === this.seq;
  }

  /** Invalidates every outstanding token without starting a request. */
  invalidate(): void {
    this.seq++;
  }
}
